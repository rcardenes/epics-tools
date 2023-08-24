use std::ffi::CStr;
use std::time::Duration;
use std::{ffi::CString, fmt::Debug};

use chrono::{DateTime, Local};
use clap::{arg, Command};
use epics_ca::types::{EpicsEnum, EpicsString};
use epics_ca::{
    request,
    types::{EpicsTimeStamp, FieldId, Value},
    Channel, Context,
};

use epics_tools::config::DEFAULT_WAIT_TIME;
use epics_tools::{UnifiedError, UnifiedResult};
use futures::TryFutureExt;
use tokio::{select, time::sleep};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const PKG_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

struct Config {
    names: Vec<String>,
    wait_time: f32,
    // Flags
    _asynchronous: bool,
    terse: bool,
    wide: bool,
}

//  Taken from the EPICS CA Reference
// DBR_CHAR 	dbr_char_t 	8 bit character
// DBR_SHORT 	dbr_short_t 	16 bit integer
// DBR_ENUM 	dbr_enum_t 	16 bit unsigned integer
// DBR_LONG 	dbr_long_t 	32 bit signed integer
// DBR_FLOAT 	dbr_float_t 	32 bit IEEE floating point
// DBR_DOUBLE 	dbr_double_t 	64 bit IEEE floating point
// DBR_STRING 	dbr_string_t 	40 character string

#[derive(Debug)]
enum RawValue {
    // Scalar
    Char(request::Time<u8>),
    Short(request::Time<i16>),
    Long(request::Time<i32>),
    Enum(request::Time<EpicsEnum>),
    Float(request::Time<f32>),
    Double(request::Time<f64>),
    String(request::Time<EpicsString>),
    // Arrays
    ShortArray(Box<request::Time<[i16]>>),
    LongArray(Box<request::Time<[i32]>>),
    FloatArray(Box<request::Time<[f32]>>),
    DoubleArray(Box<request::Time<[f64]>>),
    StringArray(Box<request::Time<[EpicsString]>>),
}

macro_rules! impl_get_stamp {
    ($op:ident, $( $name:ident ),+) => {
        match $op {
            $(RawValue::$name(val) => val.stamp,)+
        }
    };
}

impl RawValue {
    fn get_stamp(&self) -> EpicsTimeStamp {
        impl_get_stamp!(
            self,
            Char,
            Short,
            Long,
            Float,
            Double,
            Enum,
            String,
            ShortArray,
            LongArray,
            FloatArray,
            DoubleArray,
            StringArray
        )
        // match self {
        //     // RawValue::Char(val) => val.stamp,
        //     RawValue::Short(val) => val.stamp,
        //     RawValue::Long(val) => val.stamp,
        //     RawValue::Float(val) => val.stamp,
        //     RawValue::Double(val) => val.stamp,
        //     RawValue::Enum(val) => val.stamp,
        //     RawValue::String(val) => val.stamp,
        //     RawValue::ShortArray(val) => val.stamp,
        //     RawValue::LongArray(val) => val.stamp,
        //     RawValue::FloatArray(val) => val.stamp,
        //     RawValue::DoubleArray(val) => val.stamp,
        //     RawValue::StringArray(val) => val.stamp,
        // }
    }

    fn format_scalar(&self) -> String {
        match self {
            RawValue::Short(val) => format!("{}", val.value),
            RawValue::Long(val) => format!("{}", val.value),
            RawValue::Float(val) => format!("{:.5}", val.value),
            RawValue::Double(val) => format!("{:.5}", val.value),
            RawValue::Enum(val) => format!("{}", val.value.0),
            RawValue::String(val) => val.value.to_string_lossy().to_string(),
            _ => format!("<formatting not implemented yet for {self:#?}>"),
        }
    }

    fn format_array(&self, padding: usize) -> String {
        fn format_array<T>(padding: usize, data: &request::Time<[T]>) -> String
        where
            T: ToString,
            [T]: epics_ca::types::Value,
        {
            let mut rest: Vec<_> = data.value.iter().map(|d| d.to_string()).collect();
            for _ in 0..(padding - rest.len()) {
                rest.push("0".into());
            }
            rest.join(" ").to_string()
        }

        match self {
            RawValue::LongArray(val) => format_array(padding, val),
            _ => format!("<formatting not implemented yet for {self:#?}>"),
        }
    }
}

#[derive(Debug)]
struct Info {
    name: String,
    elements: usize,
    value: RawValue,
}

impl Info {
    pub fn new(name: String, elements: usize, value: RawValue) -> Self {
        Info {
            name,
            elements,
            value,
        }
    }

    pub fn is_scalar(&self) -> bool {
        self.elements == 1
    }

    fn format_scalar(&self) -> String {
        self.value.format_scalar()
    }

    fn format_array(&self, count: usize) -> String {
        self.value.format_array(count)
    }

    fn format_stamp(&self) -> String {
        let stamp: DateTime<Local> = self.value.get_stamp().to_system().into();
        format!("{}", stamp.format("%F %T%.6f"))
    }
}

fn wait_time_in_range(s: &str) -> Result<f32, String> {
    let time: f32 = s
        .parse()
        .map_err(|_| "The wait time must be a real number".to_string())?;
    if time > 0.0 {
        Ok(time)
    } else {
        Err("Wait time must be a positive value".into())
    }
}

async fn get_arguments() -> UnifiedResult<Config> {
    let matches = Command::new(PKG_NAME)
        .version(PKG_VERSION)
        .author(PKG_AUTHORS)
        .about("Rust caget")
        .args([
            arg!(wait: -w <sec> "-w <sec>: Wait time, specifies CA timeout")
                .default_value(DEFAULT_WAIT_TIME)
                .value_parser(wait_time_in_range),
            arg!(asget: -c "Asynchronous get (use a callback and wait for completion)"),
            arg!(terse: -t "Terse mode - print only value, without name"),
            arg!(wide: -a "Wide mode \"name timestamp value stat sevr\""),
            arg!(names: <PV> ... "PV names"),
        ])
        .get_matches();

    let names = matches
        .get_many::<String>("names")
        .unwrap()
        .cloned()
        .collect();
    let wait_time = *matches.get_one::<f32>("wait").unwrap();

    Ok(Config {
        names,
        wait_time,
        _asynchronous: matches.get_flag("asget"),
        terse: matches.get_flag("terse"),
        wide: matches.get_flag("wide"),
    })
}

fn get_channels(ctx: &Context, config: &Config) -> UnifiedResult<Vec<Channel>> {
    let mut errors = vec![];

    let channels: Vec<_> = config
        .names
        .iter()
        .map(|name| match CString::new(name.as_str()) {
            Ok(pvname) => Channel::new(ctx, &pvname).map_err(UnifiedError::CaError),
            Err(error) => Err(UnifiedError::Misc(format!("{error}"))),
        })
        .filter_map(|r| r.map_err(|e| errors.push(e)).ok())
        .collect();

    Ok(channels)
}

async fn wait_connect(channels: &mut [Channel], timeout: u64) -> UnifiedResult<()> {
    let connected: Vec<_> = channels.iter_mut().map(|ch| ch.connected()).collect();
    let sleeper = sleep(Duration::from_millis(timeout));

    /*
       This is a bit of Rust's async black magic (pinned vs. unpinned data), having to
       do with data migration across threads. It makes sense once you read about it,
       though.
    */
    tokio::pin!(sleeper);

    select! {
        _ = futures::future::join_all(connected) => Ok(()),
        () = &mut sleeper =>
            Err(UnifiedError::Misc("Channel connect timed out: some PV(s) not found.".into())),
    }
}

pub async fn connect<V: Value + ?Sized>(
    ctx: &Context,
    name: &CStr,
) -> Result<epics_ca::ValueChannel<V>, epics_ca::error::Error> {
    let mut chan = Channel::new(ctx, name)?;
    chan.connected().await;
    let typed = chan.into_typed::<V>().map_err(|(err, _)| err)?;
    Ok(typed.into_value())
}

macro_rules! get_value {
    ($channel:expr, $V:ty, $F:expr) => {
        $F($channel
            .into_typed::<$V>()
            .map_err(|(err, _)| UnifiedError::CaError(err))?
            .get::<request::Time<$V>>()
            .await
            .map_err(|err| UnifiedError::CaError(err))?)
    };
}

macro_rules! get_array {
    ($channel:expr, $V:ty, $F:expr) => {
        $F($channel
            .into_typed::<$V>()
            .map_err(|(err, _)| UnifiedError::CaError(err))?
            .get_boxed::<request::Time<$V>>()
            .await
            .map_err(|err| UnifiedError::CaError(err))?)
    };
}

async fn grab_info(_config: &Config, channels: Vec<Channel>) -> UnifiedResult<Vec<Info>> {
    let mut ret = vec![];

    for ch in channels {
        let count = ch.element_count().unwrap();
        ret.push(Info::new(
            ch.name().to_string_lossy().into(),
            count,
            if count == 1 {
                match ch.field_type().unwrap() {
                    FieldId::Short => get_value!(ch, i16, RawValue::Short),
                    FieldId::Float => get_value!(ch, f32, RawValue::Float),
                    FieldId::Enum => get_value!(ch, EpicsEnum, RawValue::Enum),
                    FieldId::Char => get_value!(ch, u8, RawValue::Char),
                    FieldId::Long => get_value!(ch, i32, RawValue::Long),
                    FieldId::Double => get_value!(ch, f64, RawValue::Double),
                    FieldId::String => get_value!(ch, EpicsString, RawValue::String),
                }
            } else {
                match ch.field_type().unwrap() {
                    FieldId::Short => get_array!(ch, [i16], RawValue::ShortArray),
                    FieldId::Float => get_array!(ch, [f32], RawValue::FloatArray),
                    FieldId::Long => get_array!(ch, [i32], RawValue::LongArray),
                    FieldId::Double => get_array!(ch, [f64], RawValue::DoubleArray),
                    FieldId::String => get_array!(ch, [EpicsString], RawValue::StringArray),
                    _ => unimplemented!(),
                }
            },
        ));
    }
    Ok(ret)
}

fn print_formatted(chan_info: &Info, config: &Config) {
    let mut components = vec![];
    let scalar = chan_info.is_scalar();

    if !config.terse {
        components.push(if scalar {
            format!("{:<30}", chan_info.name)
        } else {
            chan_info.name.to_string()
        });
    }

    if config.wide {
        components.push(chan_info.format_stamp());
    }

    if !scalar {
        components.push(format!("{}", chan_info.elements));
    }
    components.push(if scalar {
        chan_info.format_scalar()
    } else {
        chan_info.format_array(chan_info.elements)
    });

    println!("{}", components.join(" "));
}

async fn run(config: Config) -> UnifiedResult<()> {
    let timeout = (config.wait_time * 1000.0) as u64;
    let ctx = Context::new().map_err(UnifiedError::CaError)?;
    let mut channels = get_channels(&ctx, &config)?;
    // if config.asynchronous {
    //     print_async(config, channels);
    // } else {
    //     print_sync(config, channels);
    // }
    wait_connect(&mut channels, timeout).await?;
    let info = grab_info(&config, channels).await?;

    for ch in info {
        print_formatted(&ch, &config);
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = get_arguments().and_then(run).await {
        match e {
            UnifiedError::Misc(msg) => eprintln!("{msg}"),
            _ => eprintln!("{e:?}"),
        }
    }
}

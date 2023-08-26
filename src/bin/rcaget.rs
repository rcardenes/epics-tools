use epics_tools::{wait_connect, get_channels, grab_info};
use std::ffi::CStr;
use std::time::Duration;

use clap::{arg, Command};
use epics_ca::{
    types::Value,
    Channel, Context,
};
use epics_tools::{
    config::{DEFAULT_WAIT_TIME, wait_time_in_range},
    types::Info,
    UnifiedError,
    UnifiedResult
};

use futures::TryFutureExt;
use tokio::{select, task::JoinSet, time::sleep};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const PKG_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

struct Config {
    names: Vec<String>,
    wait_time: f32,
    // Flags
    asynchronous: bool,
    terse: bool,
    wide: bool,
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
        asynchronous: matches.get_flag("asget"),
        terse: matches.get_flag("terse"),
        wide: matches.get_flag("wide"),
    })
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
        chan_info.format_array_full()
    });

    println!("{}", components.join(" "));
}

async fn collect_sync(mut channels: Vec<Channel>, timeout: u64) -> UnifiedResult<Vec<Info>> {
    wait_connect(&mut channels, timeout).await?;

    let mut result = vec![];
    for ch in channels {
        result.push(grab_info(ch).await?);
    }
    Ok(result)
}

async fn collect_async(channels: Vec<Channel>, timeout: u64) -> UnifiedResult<Vec<Info>> {
    let mut set = JoinSet::new();

    for mut ch in channels {
        set.spawn(async move {
            let sleeper = sleep(Duration::from_millis(timeout));
            tokio::pin!(sleeper);

            select! {
                () = ch.connected() => Ok(()),
                () = &mut sleeper =>
                    Err(UnifiedError::Misc("Channel connect timed out: some PV(s) not found.".into())),
            }?;
            grab_info(ch).await
        });
    }

    let mut result = vec![];

    while let Some(task_res) = set.join_next().await {
        if let Ok(res) = task_res {
            result.push(res?);
        }
    }

    Ok(result)
}

async fn run(config: Config) -> UnifiedResult<()> {
    let timeout = (config.wait_time * 1000.0) as u64;
    let ctx = Context::new().map_err(UnifiedError::CaError)?;
    let channels = get_channels(&ctx, &config.names)?;

    let info = if config.asynchronous {
        collect_async(channels, timeout).await?
    } else {
        collect_sync(channels, timeout).await?
    };

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

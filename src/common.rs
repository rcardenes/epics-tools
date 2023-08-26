use crate::types::RawValue;
use std::{ffi::CString, time::Duration};

use epics_ca::{
    Context,
    Channel,
    request,
    types::{EpicsEnum, EpicsString, FieldId}
};
use futures::future::join_all;
use tokio::{time::sleep, select};

use crate::{UnifiedResult, UnifiedError, types::Info};


pub fn get_channels(ctx: &Context, names: &[String]) -> UnifiedResult<Vec<Channel>> {
    let mut errors = vec![];

    let channels: Vec<_> = names
        .iter()
        .map(|name| match CString::new(name.as_str()) {
            Ok(pvname) => Channel::new(ctx, &pvname).map_err(UnifiedError::CaError),
            Err(error) => Err(UnifiedError::Misc(format!("{error}"))),
        })
        .filter_map(|r| r.map_err(|e| errors.push(e)).ok())
        .collect();

    Ok(channels)
}

pub async fn wait_connect(channels: &mut [Channel], timeout: u64) -> UnifiedResult<()> {
    let connected: Vec<_> = channels.iter_mut().map(|ch| ch.connected()).collect();
    let sleeper = sleep(Duration::from_millis(timeout));

    /*
       This is a bit of Rust's async black magic (pinned vs. unpinned data), having to
       do with data migration across threads. It makes sense once you read about it,
       though.
    */
    tokio::pin!(sleeper);

    select! {
        _ = join_all(connected) => Ok(()),
        () = &mut sleeper =>
            Err(UnifiedError::Misc("Channel connect timed out: some PV(s) not found.".into())),
    }
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

pub async fn grab_info(channel: Channel) -> UnifiedResult<Info> {
    let count = channel.element_count().unwrap();
    let name = channel.name().to_string_lossy().to_string();
    let tp = channel.field_type().unwrap();

    Ok(Info::new(
        name,
        count,
        if count == 1 {
            match tp {
                FieldId::Short => get_value!(channel, i16, RawValue::Short),
                FieldId::Float => get_value!(channel, f32, RawValue::Float),
                FieldId::Enum => get_value!(channel, EpicsEnum, RawValue::Enum),
                FieldId::Char => get_value!(channel, u8, RawValue::Char),
                FieldId::Long => get_value!(channel, i32, RawValue::Long),
                FieldId::Double => get_value!(channel, f64, RawValue::Double),
                FieldId::String => get_value!(channel, EpicsString, RawValue::String),
            }
        } else {
            match tp {
                FieldId::Short => get_array!(channel, [i16], RawValue::ShortArray),
                FieldId::Float => get_array!(channel, [f32], RawValue::FloatArray),
                FieldId::Long => get_array!(channel, [i32], RawValue::LongArray),
                FieldId::Double => get_array!(channel, [f64], RawValue::DoubleArray),
                FieldId::String => get_array!(channel, [EpicsString], RawValue::StringArray),
                _ => unimplemented!(),
            }
        },
    ))
}
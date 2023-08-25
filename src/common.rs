use std::{ffi::CString, time::Duration};

use epics_ca::{Context, Channel};
use futures::future::join_all;
use tokio::{time::sleep, select};

use crate::{UnifiedResult, UnifiedError};


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

pub const DEFAULT_WAIT_TIME: &str = "1.0";
pub const DEFAULT_TIMESTAMP: TimestampKind = TimestampKind::CAServer;

pub enum TimestampKind {
    CAServer,
    CAClient,
    Incremental,
    IncrementalByChannel,
    No,
    Relative,
}

pub fn wait_time_in_range(s: &str) -> Result<f32, String> {
    let time: f32 = s
        .parse()
        .map_err(|_| "The wait time must be a real number".to_string())?;
    if time > 0.0 {
        Ok(time)
    } else {
        Err("Wait time must be a positive value".into())
    }
}

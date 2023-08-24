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

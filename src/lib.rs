pub mod config;
pub mod startup;

pub type UnifiedResult<T> = Result<T, UnifiedError>;

#[derive(Debug)]
pub enum UnifiedError {
    CaError(epics_ca::Error),
    Misc(String),
}

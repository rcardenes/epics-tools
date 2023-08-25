pub mod common;
pub mod config;
pub mod types;

pub use common::*;

pub type UnifiedResult<T> = Result<T, UnifiedError>;

#[derive(Debug)]
pub enum UnifiedError {
    CaError(epics_ca::Error),
    Misc(String),
}

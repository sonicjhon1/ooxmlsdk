use rootcause::Report;
use thiserror::Error;

pub type BuildErrorReport = Report<BuildError>;

#[derive(Error, Debug)]
pub enum BuildError {
    #[error("I/O error: {_0}")]
    IOError(#[from] std::io::Error),
    #[error("Syn error: {_0}")]
    SynError(#[from] syn::Error),
    #[error("Expected {_0} to exist, but found None")]
    HashMapExpectedSomeError(String),
}

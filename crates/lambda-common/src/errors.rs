use thiserror::Error;

#[derive(Debug, Error)]
#[error("{0}")]
pub struct UnsupportedBodyError(&'static str);

impl UnsupportedBodyError {
    pub fn new(message: &'static str) -> Self {
        Self(message)
    }
}

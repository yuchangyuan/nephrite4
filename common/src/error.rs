use std::error;
use std::fmt;
use std::io;
use core::result;

use serde_yaml;
use serde_cbor;

#[derive(Debug)]
pub enum Error {
    Simple(String),
    IO(io::Error),
    YAML(serde_yaml::Error),
    CBOR(serde_cbor::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Simple(msg) => write!(f, "SimpleError: {}", msg),
            Error::YAML(e) => e.fmt(f),
            Error::CBOR(e) => e.fmt(f),
            Error::IO(e) => e.fmt(f)
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Simple(_) => None,
            Error::YAML(e) => Some(e),
            Error::CBOR(e) => Some(e),
            Error::IO(e) => Some(e),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IO(e)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(e: serde_yaml::Error) -> Self {
        Error::YAML(e)
    }
}

impl From<serde_cbor::Error> for Error {
    fn from(e: serde_cbor::Error) -> Self {
        Error::CBOR(e)
    }
}

pub fn err<T>(msg: &str) -> Result<T> {
    Err(err_simple(msg))
}

pub fn err_simple(msg: &str) -> Error {
    Error::Simple(msg.to_string())
}

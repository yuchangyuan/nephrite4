use nephrite4_common::error as cerr;
use postgres;
use std::{io, fmt, error};
use core::result;

#[derive(Debug)]
pub enum Error {
    Simple(String),
    IO(io::Error),
    Pg(postgres::Error),
    CErr(cerr::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Simple(msg) => write!(f, "SimpleError: {}", msg),
            Error::CErr(e) => e.fmt(f),
            Error::Pg(e) => e.fmt(f),
            Error::IO(e) => e.fmt(f)
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::Simple(_) => None,
            Error::CErr(e) => Some(e),
            Error::Pg(e) => Some(e),
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

impl From<cerr::Error> for Error {
    fn from(e: cerr::Error) -> Self {
        match e {
            cerr::Error::Simple(msg) => Error::Simple(msg),
            cerr::Error::IO(e) => Error::IO(e),
            _ => Error::CErr(e),
        }
    }
}

pub fn err<T>(msg: &str) -> Result<T> {
    Err(err_simple(msg))
}

pub fn err_simple(msg: &str) -> Error {
    Error::Simple(msg.to_string())
}


impl From<postgres::Error> for Error {
    fn from(e: postgres::Error) -> Self {
        Error::Pg(e)
    }
}

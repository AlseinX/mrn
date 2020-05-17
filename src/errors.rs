use std::{
    error,
    ffi::{OsStr, OsString},
    fmt::{Debug, Display},
    ops::Deref,
};

use Error::*;

pub trait Expect {
    type T;
    type E;
    fn expect_fn<S: Display, F: Fn(Self::E) -> S>(self, msg: F) -> Self::T;
}

impl<T, E> Expect for Result<T, E> {
    type T = T;
    type E = E;
    #[inline]
    fn expect_fn<S: Display, F: Fn(Self::E) -> S>(self, msg: F) -> Self::T {
        match self {
            Ok(x) => x,
            Err(e) => panic!("{}", msg(e)),
        }
    }
}

pub trait ExceptDisplay {
    type T;
    fn expect_display(self) -> Self::T;
}

impl<T, E: Display> ExceptDisplay for Result<T, E> {
    type T = T;
    #[inline]
    fn expect_display(self) -> Self::T {
        match self {
            Ok(value) => value,
            Err(err) => {
                panic!("{}", err);
            }
        }
    }
}

impl<T> ExceptDisplay for Option<T> {
    type T = T;
    #[inline]
    fn expect_display(self) -> Self::T {
        match self {
            Some(value) => value,
            None => panic!(),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidFileName(OsString),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(
            &*match self {
                Error::InvalidFileName(value) => {
                    format!("File \"{}\" has an invalid name.", value.to_string_lossy())
                }
            },
            f,
        )
    }
}

impl error::Error for Error {}

pub trait ConvertToUtf8 {
    type S: Deref<Target = str>;
    fn convert_to_utf_8(self) -> Result<Self::S, Error>;
}

impl ConvertToUtf8 for OsString {
    type S = String;
    fn convert_to_utf_8(self) -> Result<String, Error> {
        match self.into_string() {
            Ok(value) => Ok(value),
            Err(os_string) => Err(InvalidFileName(os_string)),
        }
    }
}
impl<'a> ConvertToUtf8 for &'a OsStr {
    type S = &'a str;
    fn convert_to_utf_8(self) -> Result<&'a str, Error> {
        match self.to_str() {
            Some(value) => Ok(value),
            None => Err(InvalidFileName(self.to_os_string())),
        }
    }
}

#[macro_export]
macro_rules! expect {
    ($result:expr, |$err:ident| $error:expr) => {
        match $result {
            Ok(v) => v,
            Err($err) => {
                return core::result::Result::Err($error);
            }
        }
    };
    ($result:expr, $error:expr) => {
        match $result {
            Ok(v) => v,
            Err(_) => {
                return core::result::Result::Err($error);
            }
        }
    };
    ($result:expr, None => $error:expr) => {
        match $result {
            Some(v) => v,
            None => {
                return core::result::Result::Err($error);
            }
        }
    };
    ($result:expr) => {
        match $result {
            Ok(v) => v,
            Err(e) => {
                return core::result::Result::Err(e);
            }
        }
    };
}

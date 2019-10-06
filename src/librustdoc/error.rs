use crate::docfs::PathError;
use std::error;
use std::fmt::{self, Formatter};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Error {
    pub file: PathBuf,
    pub error: io::Error,
}

impl error::Error for Error {
    fn description(&self) -> &str {
        self.error.description()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let file = self.file.display().to_string();
        if file.is_empty() {
            write!(f, "{}", self.error)
        } else {
            write!(f, "\"{}\": {}", self.file.display(), self.error)
        }
    }
}

impl PathError for Error {
    fn new<P: AsRef<Path>>(e: io::Error, path: P) -> Error {
        Error {
            file: path.as_ref().to_path_buf(),
            error: e,
        }
    }
}

#[macro_export]
macro_rules! try_none {
    ($e:expr, $file:expr) => ({
        use std::io;
        match $e {
            Some(e) => e,
            None => return Err(Error::new(io::Error::new(io::ErrorKind::Other, "not found"),
                                          $file))
        }
    })
}

#[macro_export]
macro_rules! try_err {
    ($e:expr, $file:expr) => ({
        match $e {
            Ok(e) => e,
            Err(e) => return Err(Error::new(e, $file)),
        }
    })
}
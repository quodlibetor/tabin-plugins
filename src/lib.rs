//! Utitilty for writing nagios-style check scripts/plugins
//!
//! There are three things:
//!
//! * The `Status` enum for representing health status
//! * The `procfs` module, which contains rusty representations of some files
//!   from /proc
//! * A few scripts in the bin directory, which contain actual
//!   nagios-compatible scripts
//!
//! TODOs include
//!
//! * Nice logging, including some standard formats -- json and human would be
//!   nice
//! * Some way of easily standardizing command-line args
//! * Much of the code is hideous, and should not be

#[macro_use]
extern crate lazy_static;

use std::fmt;
use std::process;
use std::str::FromStr;

use serde::Deserialize;

pub mod linux;
pub mod procfs;
pub mod scripts;
pub mod sys;

/// All errors are TabinErrors
#[derive(Debug)]
pub enum TabinError {
    /// Represents an incorrect value passed in to a function
    UnknownValue(String),
}

impl fmt::Display for TabinError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            &TabinError::UnknownValue(ref msg) => write!(f, "Unknown Value: {}", msg),
        }
    }
}

/// All results are TabinResults
pub type TabinResult<T> = Result<T, TabinError>;

/// Represent the nagios-ish error status of a script.
#[must_use]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Deserialize)]
pub enum Status {
    /// Unexpected result
    Unknown,
    /// Everything is fine
    Ok,
    /// Something is weird, but don't necessary call anybody
    Warning,
    /// OMG CALL SOMEONE I DON'T CARE WHAT TIME IT IS
    Critical,
}

impl Status {
    /// Exit with a return code that indicates the state of the system
    pub fn exit(&self) -> ! {
        use self::Status::*;
        match *self {
            Ok => process::exit(0),
            Warning => process::exit(1),
            Critical => process::exit(2),
            Unknown => process::exit(3),
        }
    }

    /// The legal values for `from_str`
    pub fn str_values() -> [&'static str; 4] {
        ["ok", "warning", "critical", "unknown"]
    }
}

impl FromStr for Status {
    type Err = TabinError;

    /// Primarily useful to construct from argv
    fn from_str(s: &str) -> TabinResult<Status> {
        use crate::Status::{Critical, Unknown, Warning};
        match s {
            "ok" => Ok(Status::Ok),
            "warning" => Ok(Warning),
            "critical" => Ok(Critical),
            "unknown" => Ok(Unknown),
            _ => Err(TabinError::UnknownValue(format!(
                "Unexpected exit status: {}",
                s
            ))),
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use crate::Status::*;
        let msg = match *self {
            Ok => "OK",
            Unknown => "UNKNOWN",
            Warning => "WARNING",
            Critical => "CRITICAL",
        };
        write!(f, "{}", msg)
    }
}

#[test]
fn comparison_is_as_expected() {
    use crate::Status::*;
    assert!(Ok < Critical);
    assert!(Ok < Warning);
    assert_eq!(std::cmp::max(Warning, Critical), Critical)
}

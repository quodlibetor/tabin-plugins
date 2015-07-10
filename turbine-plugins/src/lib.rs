//! Utitilty for writing nagios-style check scripts/plugins
//!
//! Right now this is just the `ExitStatus` struct, useful for making sure that
//! you exit correctly.


use std::process;
use std::cmp::{Ord};

/// All errors are TurbineErrors
#[derive(Debug)]
pub enum TurbineError {
    /// Represents an incorrect value passed in to a function
    UnknownValue(String)
}

/// All results are TurbineResults
pub type TurbineResult<T> = Result<T, TurbineError>;

/// Represent the nagios-ish error status of a script.
#[must_use]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExitStatus {
    /// Unexpected result
    Unknown,
    /// Everything is fine
    Ok,
    /// Something is weird, but don't necessary call anybody
    Warning,
    /// OMG CALL SOMEONE I DON'T CARE WHAT TIME IT IS
    Critical,
}

impl ExitStatus {
    /// Exit with a return code that indicates the state of the system
    pub fn exit(self) -> ! {
        use self::ExitStatus::*;
        match self {
            Ok => process::exit(0),
            Warning => process::exit(1),
            Critical => process::exit(2),
            Unknown => process::exit(3)
        }
    }

    /// Primarily useful to construct from argv
    pub fn from_str(s: &str) -> TurbineResult<ExitStatus> {
        use ExitStatus::{Warning, Critical, Unknown};
        match s {
            "ok" => Ok(ExitStatus::Ok),
            "warning" => Ok(Warning),
            "critical" => Ok(Critical),
            "unknown" => Ok(Unknown),
            _ => Err(TurbineError::UnknownValue(format!("Unexpected exit status: {}", s)))
        }
    }

    /// The legal values for `from_str`
    pub fn str_values() -> [&'static str; 4] {
        ["ok", "warn", "critical", "unknown"]
    }
}

#[test]
fn comparison_is_as_expected() {
    use ExitStatus::*;
    assert!(Ok < Critical);
    assert!(Ok < Warning);
    assert_eq!(std::cmp::max(Warning, Critical), Critical)
}

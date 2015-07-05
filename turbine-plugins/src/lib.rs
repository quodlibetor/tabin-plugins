use std::process;

#[must_use]
#[derive(Debug)]
pub enum ExitStatus {
    Ok,
    Warning,
    Critical,
    Unknown
}

impl ExitStatus {
    #![cfg_attr(test, allow(dead_code))]
    pub fn exit(self) -> ! {
        use self::ExitStatus::*;
        match self {
            Ok => process::exit(0),
            Warning => process::exit(1),
            Critical => process::exit(2),
            Unknown => process::exit(3)
        }
    }

    pub fn from_str(s: &str) -> ExitStatus {
        use ExitStatus::*;
        match s {
            "ok" => Ok,
            "warning" => Warning,
            "critical" => Critical,
            "unknown" => Unknown,
            _ => panic!("Unexpected exit status: {}", s)
        }
    }

    pub fn str_values() -> [&'static str; 4] {
        ["ok", "warn", "critical", "unknown"]
    }
}

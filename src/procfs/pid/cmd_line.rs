use std::fmt;
use std::fs::File;
use std::io::Read;

use procfs::Result;

/// The visibule command line for a process
#[derive(Clone, Debug, Default)]
pub struct CmdLine {
    /// The raw parts of the command line
    ///
    /// The vec of arguments that started the process
    pub raw: Vec<String>,
}

impl CmdLine {
    pub fn from_pid<P: fmt::Display>(pid: P) -> Result<CmdLine> {
        let path_str = format!("/proc/{}/cmdline", pid);
        let mut f = try!(File::open(&path_str));
        let mut s = String::new();
        try!(f.read_to_string(&mut s));
        Ok(CmdLine {
            raw: s
                .split('\0')
                .map(String::from)
                .filter(|arg| !arg.is_empty())
                .collect(),
        })
    }

    pub fn len(&self) -> usize {
        self.raw.len()
    }

    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    pub fn display(&self) -> String {
        self.raw.join(" ")
    }
}

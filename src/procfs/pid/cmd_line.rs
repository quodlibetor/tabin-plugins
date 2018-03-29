use std::io::Read;
use std::fmt;
use std::fs::File;

use procfs::Result;

/// The visibule command line for a process
#[derive(Debug)]
pub struct CmdLine {
    line: Vec<String>,
}

impl CmdLine {
    pub fn from_pid<P: fmt::Display>(pid: P) -> Result<CmdLine> {
        let path_str = format!("/proc/{}/cmdline", pid);
        let mut f = try!(File::open(&path_str));
        let mut s = String::new();
        try!(f.read_to_string(&mut s));
        Ok(CmdLine {
            line: s.split('\0')
                .map(String::from)
                .filter(|arg| !arg.is_empty())
                .collect(),
        })
    }

    pub fn len(&self) -> usize {
        self.line.len()
    }

    pub fn is_empty(&self) -> bool {
        self.line.is_empty()
    }

    pub fn display(&self) -> String {
        self.line.join(" ")
    }
}

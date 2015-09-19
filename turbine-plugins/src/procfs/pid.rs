//! Data structures related to the /proc/<pid>/* files

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;

use super::ProcFsError;
use super::Result;

pub struct Process {
    pub stat: Stat,
    pub cmdline: CmdLine,
}

impl Process {
    pub fn from_pid<P: fmt::Display + Copy>(p: P) -> Result<Process> {
        Ok(Process {
            stat: try!(Stat::from_pid(p)),
            cmdline: try!(CmdLine::from_pid(p)),
        })
    }

    pub fn useful_cmdline(&self) -> String {
        pid::CmdLine::from_pid(usage.proc_stat.pid)
            .map(|cmd| {
                let c = String::from(cmd);
                if c.len() > 0 {
                    c
                } else {
                    usage.proc_stat.comm.clone()
                }
            })
            .unwrap_or(usage.proc_stat.comm.clone())
    }
}

/// The status of a process
///
/// This represents much of the information in /proc/[pid]/stat
#[derive(Debug)]
pub struct Stat {
    pub pid: i32,
    pub comm: String,
    pub state: String,
    pub ppid: i32,
    pub pgrp: i32,
    pub session: i32,
    pub tty_nr: i32,
    pub tpgid: i32,
    pub flags: u32,
    pub minflt: u64,
    pub cminflt: u64,
    pub majflt: u64,
    pub cmajflt: u64,
    pub utime: u64,
    pub stime: u64,
    pub cutime: i64,
    pub cstime: i64,
    pub priority: i64,
    pub nice: i64,
    pub num_threads: i64,
    pub starttime: u64,
    pub vsize: u64,
    pub rss: i64
}

impl Stat {
    pub fn from_pid<P: fmt::Display>(pid: P) -> Result<Stat> {
        let path_str = format!("/proc/{}/stat", pid);
        let mut f = try!(File::open(&path_str));
        let mut s = String::new();
        try!(f.read_to_string(&mut s));
        s.parse()
    }
}

impl Default for Stat {
    fn default() -> Stat {
        Stat {
            pid: 0, comm: "init".to_string(), state: "R".to_string(), ppid: 0,
            pgrp: 0, session: 0, tty_nr: 0, tpgid: 0, flags: 0, minflt: 0,
            cminflt: 0, majflt: 0, cmajflt: 0, utime: 0, stime: 0, cutime: 0,
            cstime: 0, priority: 0, nice: 0, num_threads: 0, starttime: 0,
            vsize: 0, rss: 0
        }
    }
}

impl FromStr for Stat {
    type Err = ProcFsError;
    /// Parse the results of /proc/[pid]/stat into a `Stat`
    fn from_str(s: &str) -> Result<Stat> {
        let (pid, comm, state, ppid, pgrp, session, tty_nr, tpgid, flags, minflt, cminflt, majflt,
             cmajflt, utime, stime, cutime, cstime, priority, nice, num_threads, starttime, vsize,
             rss) = scan_fmt!(
            s,
            "{d} ({[^)]}) {} {} {} {} {} {} {d} {d} {d} {d} {d} {d} {d} {} {} {} {} {} 0 {d} {d} {}",
            i32,                    // pid
            String,                 // comm
            String,                 // state
            i32,                    // ppid
            i32,                    // pgrp
            i32,                    // session
            i32,                    // tty_nr
            i32,                    // tpgid
            u32,                    // flags
            u64,                    // minflt
            u64,                    // cminflt
            u64,                    // majflt
            u64,                    // cmajflt
            u64,                    // utime
            u64,                    // stime
            i64,                    // cutime (children usertime)
            i64,                    // cstime
            i64,                    // priority
            i64,                    // nice
            i64,                    // num_threads
            // itrealvalue (always 0)
            u64,                    // starttime FIXME: should be long long int
            u64,                    // vsize
            i64                     // rss
                );
        Ok(Stat {
            pid: pid.expect("unable to parse pid."),
            comm: comm.expect("unable to parse comm."),
            state: state.expect("unable to parse state."),
            ppid: ppid.expect("unable to parse ppid."),
            pgrp: pgrp.expect("unable to parse pgrp."),
            session: session.expect("unable to parse session."),
            tty_nr: tty_nr.expect("unable to parse tty_nr."),
            tpgid: tpgid.expect("unable to parse tpgid."),
            flags: flags.expect("unable to parse flags."),
            minflt: minflt.expect("unable to parse minflt."),
            cminflt: cminflt.expect("unable to parse cminflt."),
            majflt: majflt.expect("unable to parse majflt."),
            cmajflt: cmajflt.expect("unable to parse cmajflt."),
            utime: utime.expect("unable to parse utime."),
            stime: stime.expect("unable to parse stime."),
            cutime: cutime.expect("unable to parse cutime."),
            cstime: cstime.expect("unable to parse cstime."),
            priority: priority.expect("unable to parse priority."),
            nice: nice.expect("unable to parse nice."),
            num_threads: num_threads.expect("unable to parse num_threads."),
            starttime: starttime.expect("unable to parse starttime."),
            vsize: vsize.expect("unable to parse vsize."),
            rss: rss.expect("unable to parse rss.")
        })
    }
}

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
        //let splitted = ;
        Ok(CmdLine {
            line: s.split("\0")
                .map(|arg| String::from(arg))
                .filter(|arg| arg.len() > 0).collect(),
        })
    }
}

impl fmt::Display for CmdLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.line.join(" "))
    }
}

impl From<CmdLine> for String {
    fn from(c: CmdLine) -> String {
        format!("{}", c)
    }
}

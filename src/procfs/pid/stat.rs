use std::fmt;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;

use nix::unistd::Pid;

use linux::Jiffies;
use procfs::{ParseStatError, ParseStateError, ProcFsError, Result};

/// The status of a `Process`
///
/// This represents much of the information in `/proc/[pid]/stat`, and is
/// commonly access via a [`Process`](../struct.Process.html)
#[derive(Clone, Debug)]
pub struct Stat {
    /// The process ID
    pub pid: Pid,
    /// The filename of the executable
    pub comm: String,
    /// The state of the process
    pub state: State,
    pub ppid: Pid,
    pub pgrp: i32,
    pub session: i32,
    pub tty_nr: i32,
    pub tpgid: i32,
    pub flags: u32,
    pub minflt: u64,
    pub cminflt: u64,
    pub majflt: u64,
    pub cmajflt: u64,
    pub utime: Jiffies,
    pub stime: Jiffies,
    pub cutime: Jiffies,
    pub cstime: Jiffies,
    pub priority: i64,
    pub nice: i64,
    pub num_threads: i64,
    pub starttime: u64,
    pub vsize: u64,
    pub rss: u64,
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
            pid: Pid::from_raw(0),
            comm: "init".to_owned(),
            state: "R".parse().unwrap(),
            ppid: Pid::from_raw(0),
            pgrp: 0,
            session: 0,
            tty_nr: 0,
            tpgid: 0,
            flags: 0,
            minflt: 0,
            cminflt: 0,
            majflt: 0,
            cmajflt: 0,
            utime: Jiffies::new(0),
            stime: Jiffies::new(0),
            cutime: Jiffies::new(0),
            cstime: Jiffies::new(0),
            priority: 0,
            nice: 0,
            num_threads: 0,
            starttime: 0,
            vsize: 0,
            rss: 0,
        }
    }
}

fn field<T>(val: Option<T>, field_name: &'static str, row: &str, position: u8) -> Result<T> {
    match val {
        Some(v) => Ok(v),
        None => {
            let row = row.to_string();
            return Err(ParseStatError {
                line: row,
                field_name,
                position,
            }
            .into());
        }
    }
}

impl FromStr for Stat {
    type Err = ProcFsError;
    /// Parse the results of /proc/[pid]/stat into a `Stat`
    fn from_str(s: &str) -> Result<Stat> {
        let (
            pid,
            comm,
            state,
            ppid,
            pgrp,
            session,
            tty_nr,
            tpgid,
            flags,
            minflt,
            cminflt,
            majflt,
            cmajflt,
            utime,
            stime,
            cutime,
            cstime,
            priority,
            nice,
            num_threads,
            starttime,
            vsize,
            rss,
        ) = scan_fmt!(
            s,
            "{d} {/\\((.*)\\) /}{} {} {} {} {} {} {d} {d} {d} {d} {d} {d} {d} {} {} \
             {} {} {} 0 {d} {d} {}",
            i32,    // pid
            String, // comm
            State,  // state
            i32,    // ppid
            i32,    // pgrp
            i32,    // session
            i32,    // tty_nr
            i32,    // tpgid
            u32,    // flags
            u64,    // minflt
            u64,    // cminflt
            u64,    // majflt
            u64,    // cmajflt
            u64,    // utime
            u64,    // stime
            i64,    // cutime (children usertime)
            i64,    // cstime
            i64,    // priority
            i64,    // nice
            i64,    // num_threads
            // itrealvalue (always 0)
            u64, // starttime FIXME: should be long long int
            u64, // vsize
            u64  /* rss */
        );
        Ok(Stat {
            pid: Pid::from_raw(field(pid, "pid", s, 0)?),
            comm: field(comm, "comm", s, 1)?,
            state: field(state, "state", s, 2)?,
            ppid: Pid::from_raw(field(ppid, "ppid", s, 3)?),
            pgrp: field(pgrp, "pgrp", s, 4)?,
            session: field(session, "session", s, 5)?,
            tty_nr: field(tty_nr, "tty_nr", s, 6)?,
            tpgid: field(tpgid, "tpgid", s, 7)?,
            flags: field(flags, "flags", s, 8)?,
            minflt: field(minflt, "minflt", s, 9)?,
            cminflt: field(cminflt, "cminflt", s, 10)?,
            majflt: field(majflt, "majflt", s, 11)?,
            cmajflt: field(cmajflt, "cmajflt", s, 12)?,
            utime: field(utime, "utime", s, 13)?.into(),
            stime: field(stime, "stime", s, 14)?.into(),
            cutime: field(cutime, "cutime", s, 15)?.into(),
            cstime: field(cstime, "cstime", s, 16)?.into(),
            priority: field(priority, "priority", s, 17)?,
            nice: field(nice, "nice", s, 18)?,
            num_threads: field(num_threads, "num_threads", s, 19)?,
            starttime: field(starttime, "starttime", s, 20)?,
            vsize: field(vsize, "vsize", s, 21)?,
            rss: field(rss, "rss", s, 22)?,
        })
    }
}

/// The state of the process
///
/// See `man 5 proc` for details
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum State {
    /// `R`: Currently using the CPU
    Running,
    /// `S`: Sleeping on an interruptible wait
    Sleeping,
    /// `D`: Sleeping on an uninterruptible or disk sleep
    UninterruptibleSleep,
    /// `W`: Paging
    Waiting,
    /// `T`: Traced or Stopped on a signal
    Stopped,
    /// `Z`: No parent process has reaped this
    Zombie,
}

impl FromStr for State {
    type Err = ProcFsError;

    /// Parse from either /proc/<pid>/stat or the command line
    ///
    /// StructOpt and scan_fmt use the same trait, so we need to handle both
    /// here.
    fn from_str(s: &str) -> Result<State> {
        use self::State::*;
        match s {
            "R" | "running" => Ok(Running),
            "S" | "sleeping" => Ok(Sleeping),
            "D" | "uninteruptible-sleep" => Ok(UninterruptibleSleep),
            "W" | "waiting" => Ok(Waiting),
            "T" | "stopped" => Ok(Stopped),
            "Z" | "zombie" => Ok(Zombie),
            _ => Err(ParseStateError {
                state: s.to_string(),
            }
            .into()),
        }
    }
}

#[cfg(test)]
mod test {
    mod cpu_stat {
        use super::super::*;

        #[test]
        fn test_parse_cpuline() {
            for (i, s) in [
                "529 ((sd-proc)) S 885 885 885 0 -1 107793 24 0 0 0 0 \
                 0 0 0 20 0 1 0 7777777 111111111 647 18848888888888888888 1 1 0 \
                 0 0 0 0 4096 0 0 0 0 17 15 0 0 0 0 0 0 0 0 0 0 0 0 0",
                "47 (migration/8) S 2 0 0 0 -1 66666668 0 0 0 0 0 14 0 0 -100 0 1 \
                 0 25 0 0 18848888888888888888 0 0 0 0 0 0 0 2147483647 0 0 0 0 17 \
                 8 99 1 0 0 0 0 0 0 0 0 0 0 0",
                "122 (statsd /app/connection) S 103 103 181 0 -1 304 0626 0 \
                 0 0 605 198 0 0 20 0 10 0 71025 1230417920 11878 18848888888888888888 \
                 1 1 0 0 0 0 0 4096 16898 0 0 0 17 11 0 0 0 0 0 0 0 0 0 0 0 0 0",
            ]
            .iter()
            .enumerate()
            {
                s.parse::<Stat>().unwrap_or_else(|e| match e {
                    ProcFsError::ParseStatError(e) => panic!("line {}: {}", i, e),
                    x => panic!("unexpected error: {:?}", x),
                });
            }
        }
    }
}

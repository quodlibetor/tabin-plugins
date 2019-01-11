//! Data structures related to the `/proc/<pid>/*` files
//!
//! The `Process` struct can load everything about a running process, and
//! provides some aggregate data about them.

mod cmd_line;
mod stat;

use std::fmt;

use super::Result;

use crate::linux::{Jiffies, Ratio, PAGESIZE};

pub use self::cmd_line::CmdLine;
pub use self::stat::{Stat, State};

/// Information about a running process
#[derive(Clone, Debug, Default)]
pub struct Process {
    /// The stat info for a process
    pub stat: Stat,
    /// The command line, as revealed by the /proc fs
    pub cmdline: CmdLine,
}

impl Process {
    pub fn from_pid<P: fmt::Display + Copy>(p: P) -> Result<Process> {
        Ok(Process {
            stat: Stat::from_pid(p)?,
            cmdline: CmdLine::from_pid(p)?,
        })
    }

    pub fn useful_cmdline(&self) -> String {
        let cmd = self.cmdline.display();
        if cmd.is_empty() {
            self.stat.comm.clone()
        } else {
            cmd
        }
    }

    /// What percent this process is using
    ///
    /// First argument should be in bytes.
    pub fn percent_ram(&self, of_bytes: usize) -> f64 {
        pages_to_bytes(self.stat.rss) as f64 / of_bytes as f64 * 100.0
    }

    /// Compare this processes cpu utilization since the process representing th start time
    ///
    /// The passed-in `start_process` is the time that we are comparing
    /// against: `self` should be newer.
    ///
    /// The `total_cpu` is how many Jiffies have passed on the cpu over the
    /// same time period.
    ///
    /// # Panics
    /// If the total_cpu on start_process is higher than the total_cpu on current.
    pub fn cpu_utilization_since<'a>(
        &'a self,
        start_process: &'a Process,
        total_cpu: Jiffies,
    ) -> ProcessCpuUsage<'a> {
        let (start_ps, end_ps) = (&start_process.stat, &self.stat);
        if end_ps.utime < start_ps.utime || end_ps.stime < start_ps.stime {
            panic!("End process is before start process (arguments called in wrong order)");
        }
        let user = 100.0
            * (end_ps.utime - start_ps.utime)
                .duration()
                .ratio(&total_cpu.duration());
        let sys = 100.0
            * (end_ps.stime - start_ps.stime)
                .duration()
                .ratio(&total_cpu.duration());
        ProcessCpuUsage {
            process: &start_process,
            upercent: user,
            spercent: sys,
            total: user + sys,
        }
    }
}

/// Represent the percent CPU utilization of a specific process over a specific
/// time period
///
/// This is generated by loading `RunningProcs` twice and diffing them.
pub struct ProcessCpuUsage<'a> {
    /// The process we're reporting on
    pub process: &'a Process,
    /// Percent time spent in user mode
    pub upercent: f64,
    /// Percent time spent in system mode
    pub spercent: f64,
    /// upercent + spercent
    pub total: f64,
}

fn pages_to_bytes(pages: u64) -> u64 {
    pages * (*PAGESIZE)
}

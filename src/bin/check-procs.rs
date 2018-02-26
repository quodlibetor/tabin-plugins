//! Check running processes

#![allow(unused_variables, dead_code)]

extern crate nix;
extern crate regex;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

extern crate tabin_plugins;

use nix::unistd::{getpid, getppid, Pid};
use regex::Regex;
use structopt::StructOpt;

use tabin_plugins::Status;
use tabin_plugins::procfs::RunningProcs;
use tabin_plugins::procfs::pid::Process;

#[derive(StructOpt, Debug, Deserialize)]
/// Check that an expected number of processes are running.
struct Args {
    #[structopt(help = "Regex that command and its arguments must match")]
    pattern: String,
    #[structopt(long = "crit-under",
                help = "Error if there are fewer than this many procs matching <pattern>")]
    crit_under: Option<usize>,
    #[structopt(long = "crit-over",
                help = "Error if there are more than this many procs matching <pattern>")]
    crit_over: Option<usize>,
}

fn main() {
    let args = Args::from_args();
    let re = Regex::new(&args.pattern).unwrap_or_else(|e| {
        println!("ERROR: invalid process pattern: {}", e);
        Status::Critical.exit()
    });
    let procs = RunningProcs::currently_running().unwrap();

    let me = getpid();
    let parent = getppid();

    let matches = procs
        .0
        .into_iter()
        .filter_map(|(pid, process)| {
            if re.is_match(&process.useful_cmdline())
                && !(Pid::from_raw(pid) == me || Pid::from_raw(pid) == parent)
            {
                Some((pid, process))
            } else {
                None
            }
        })
        .collect::<Vec<(i32, Process)>>();

    let mut status = Status::Ok;
    if let Some(crit_over) = args.crit_over {
        if matches.len() > crit_over {
            status = Status::Critical;
            println!(
                "CRITICAL: there are {} process that match {:?} (greater than {})",
                matches.len(),
                args.pattern,
                crit_over
            );
        }
    };
    if let Some(crit_under) = args.crit_under {
        if matches.len() < crit_under {
            status = Status::Critical;
            println!(
                "CRITICAL: there are {} process that match {:?} (less than {})",
                matches.len(),
                args.pattern,
                crit_under
            );
        }
    }

    if status == Status::Ok {
        match (args.crit_over, args.crit_under) {
            (Some(o), Some(u)) => println!(
                "OKAY: There are {} matching procs (between {} and {})",
                matches.len(),
                o,
                u
            ),
            (Some(o), None) => println!(
                "OKAY: There are {} matching procs (less than {})",
                matches.len(),
                o
            ),
            (None, Some(u)) => println!(
                "OKAY: There are {} matching procs (greater than {})",
                matches.len(),
                u
            ),
            (None, None) => unreachable!(),
        }
    }
    if matches.len() > 0 {
        println!("INFO: Matching processes:");
        for process in matches.iter().take(20) {
            println!("[{:>5}] {}", process.0, process.1.useful_cmdline());
        }
        if matches.len() > 20 {
            println!("And {} more...", matches.len() - 20)
        }
    }
    status.exit();
}

#[cfg(test)]
mod unit {
    use super::Args;

    use structopt::StructOpt;

    #[test]
    fn validate_docstring() {
        let args = Args::from_iter(["c-p", "some.*proc", "--crit-under=1"].into_iter());
        assert_eq!(args.crit_under, Some(1));
    }
}

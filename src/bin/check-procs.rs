//! Check running processes

#![allow(unused_variables, dead_code)]

extern crate docopt;
extern crate nix;
extern crate regex;
extern crate rustc_serialize;

extern crate tabin_plugins;

use docopt::Docopt;
use nix::unistd::{getpid, getppid};
use regex::Regex;

use tabin_plugins::Status;
use tabin_plugins::procfs::RunningProcs;
use tabin_plugins::procfs::pid::Process;


static USAGE: &'static str = "
Usage:
    check-procs <pattern> (--crit-under <N> | --crit-over <N>)
    check-procs -h | --help

Check that an expected number of processes are running.

Required Arguments:

    pattern                Regex that command and its arguments must match

Options:
    -h, --help             Show this message and exit
    --crit-under=<N>       Error if there are fewer than this pattern procs
    --crit-over=<N>        Error if there are more than this pattern procs
";

#[derive(Debug, RustcDecodable)]
struct Args {
    arg_pattern: String,
    flag_crit_under: Option<usize>,
    flag_crit_over: Option<usize>
}

impl Args {
    fn parse() -> Args {
        Docopt::new(USAGE)
            .and_then(|d| d.decode())
            .unwrap_or_else(|e| e.exit())
    }
}

fn main() {
    let args = Args::parse();
    let procs = RunningProcs::currently_running().unwrap();
    let re = Regex::new(&args.arg_pattern).unwrap_or_else(|e| {
        println!("{}", e);
        Status::Critical.exit()
    });

    let me = getpid();
    let parent = getppid();

    let matches = procs.0.into_iter()
        .filter_map(
            |(pid, process): (i32, Process)|
            if re.is_match(&process.useful_cmdline()) &&
                !(pid == me || pid == parent) {
                    Some((pid, process))
                } else {
                    None
                })
        .collect::<Vec<(i32, Process)>>();

    let mut status = Status::Ok;
    if let Some(crit_over) = args.flag_crit_over {
        if matches.len() > crit_over {
            status = Status::Critical;
            println!("CRITICAL: there are {} process that match {:?} (greater than {})",
                     matches.len(), args.arg_pattern, crit_over);
        }
    };
    if let Some(crit_under) = args.flag_crit_under {
        if matches.len() < crit_under {
            status = Status::Critical;
            println!("CRITICAL: there are {} process that match {:?} (less than {})",
                     matches.len(), args.arg_pattern, crit_under);
        }
    }

    if status == Status::Ok {
        match (args.flag_crit_over, args.flag_crit_under) {
            (Some(o), Some(u)) => println!(
                "OKAY: There are {} matching procs (between {} and {})",
                matches.len(), o, u),
            (Some(o), None) => println!(
                "OKAY: There are {} matching procs (less than {})",
                matches.len(), o),
            (None, Some(u)) => println!(
                "OKAY: There are {} matching procs (greater than {})",
                matches.len(), u),
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
    use super::{Args, USAGE};

    use docopt::Docopt;

    #[test]
    fn validate_docstring() {
        let args: Args = Docopt::new(USAGE)
            .and_then(|d| d
                      .argv(vec!["c-p", "some.*proc", "--crit-under=1"].into_iter())
                      .decode())
            .unwrap();
    }
}

//! Check RAM usage of the currently-running container

use std::cmp::max;
use std::fmt;

use serde::Deserialize;
use structopt::StructOpt;

use tabin_plugins::linux::{bytes_to_human_size, pages_to_human_size};
use tabin_plugins::procfs::{LoadProcsError, MemInfo, ProcFsError, RunningProcs};
use tabin_plugins::sys::fs::cgroup::memory::{limit_in_bytes, Stat};
use tabin_plugins::Status;

/// Check the RAM usage of the currently-running container.
///
/// This must be run from inside the container to be checked.
///
/// This checks as a ratio of the limit specified in the cgroup memory limit, and
/// if there is no limit set (or the limit is greater than the total memory
/// available on the system) this checks against the total system memory.
#[derive(Deserialize, StructOpt, Debug)]
#[structopt(
    name = "check-container-ram (part of tabin-plugins)",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct Args {
    #[structopt(
        short = "w",
        long = "warn",
        help = "Percent to warn at",
        default_value = "85"
    )]
    warn: f64,
    #[structopt(
        short = "c",
        long = "crit",
        help = "Percent to go critical at",
        default_value = "95"
    )]
    crit: f64,

    #[structopt(
        long = "invalid-limit",
        default_value = "ok",
        help = "Status to consider this check if the CGroup limit is greater than \
                the system ram"
    )]
    invalid_limit: Status,
    #[structopt(
        long = "show-hogs",
        name = "count",
        help = "Show <count> most ram-intensive processes in this container.",
        default_value = "0"
    )]
    show_hogs: usize,
}

enum Limit {
    CGroup,
    System,
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Limit::CGroup => write!(f, "cgroup limit"),
            Limit::System => write!(f, "system ram"),
        }
    }
}

fn main() {
    let args: Args = Args::from_args();
    let mut status = Status::Ok;

    let mut limit = limit_in_bytes().unwrap();
    let mut limit_type = Limit::CGroup;
    let mem = MemInfo::load();
    let system_bytes = mem.total.unwrap() * 1024;
    if limit > system_bytes {
        if args.invalid_limit != Status::Ok {
            println!(
                "{}: CGroup memory limit is greater than system memory ({} > {})",
                args.invalid_limit,
                bytes_to_human_size(limit as u64),
                bytes_to_human_size(system_bytes as u64)
            );
            status = args.invalid_limit;
        }
        limit = system_bytes;
        limit_type = Limit::System;
    }

    let cgroup_stat = Stat::load().unwrap();
    let ratio = cgroup_stat.rss as f64 / limit as f64;
    let percent = ratio * 100.0;

    if percent > args.crit {
        println!(
            "CRITICAL: cgroup is using {:.1}% of {} {} (greater than {}%)",
            percent,
            bytes_to_human_size(limit as u64),
            limit_type,
            args.crit
        );
        status = Status::Critical;
    } else if percent > args.warn {
        println!(
            "WARNING: cgroup is using {:.1}% of {} {} (greater than {}%)",
            percent,
            bytes_to_human_size(limit as u64),
            limit_type,
            args.warn
        );
        status = max(status, Status::Warning);
    } else {
        println!(
            "OK: cgroup is using {:.1}% of {} {} (less than {}%)",
            percent,
            bytes_to_human_size(limit as u64),
            limit_type,
            args.warn
        );
    }

    if args.show_hogs > 0 {
        let mut load_errors = None;
        let per_proc = match RunningProcs::currently_running() {
            Ok(procs) => procs,
            Err(ProcFsError::LoadProcsError(LoadProcsError { procs, errors })) => {
                load_errors = Some(errors);
                procs
            }
            Err(err) => {
                eprintln!("UNKNOWN: Unexpected error loading procs: {}", err);
                RunningProcs::empty()
            }
        };

        let mut procs = per_proc.0.values().collect::<Vec<_>>();
        procs.sort_by(|l, r| r.stat.rss.cmp(&l.stat.rss));
        println!(
            "INFO [check-container-ram]: {} processes running, top {} ram hogs:",
            procs.len(),
            args.show_hogs
        );
        for process in procs.iter().take(args.show_hogs as usize) {
            let percent = process.percent_ram(limit);
            println!(
                "[{:>6}]{:>5.1}% {:>6}: {}",
                process.stat.pid,
                percent,
                pages_to_human_size(process.stat.rss),
                process.useful_cmdline()
            );
        }

        if let Some(errors) = load_errors {
            for err in errors {
                eprintln!("UNKNOWN: error loading process: {}", err);
            }
        }
    }
    status.exit();
}

#[cfg(test)]
mod unit {

    use super::Args;
    use structopt::StructOpt;
    use tabin_plugins::Status;

    #[test]
    fn usage_is_valid() {
        let argv: [&str; 0] = [];
        let args = Args::from_iter(argv.iter());
        assert_eq!(args.crit, 95.0);
        assert_eq!(args.invalid_limit, Status::Ok);
        let args: Args = Args::from_iter(["arg0", "--crit", "80", "--warn", "20"].iter());
        assert_eq!(args.crit, 80.0);
    }
}

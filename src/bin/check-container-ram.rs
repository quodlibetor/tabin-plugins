//! Check RAM usage of the currently-running container

extern crate docopt;
extern crate rustc_serialize;

extern crate tabin_plugins;

use std::fmt;
use std::cmp::max;

use docopt::Docopt;

use tabin_plugins::Status;
use tabin_plugins::sys::fs::cgroup::memory::{limit_in_bytes, Stat};
use tabin_plugins::linux::{bytes_to_human_size, pages_to_human_size};
use tabin_plugins::procfs::{MemInfo, RunningProcs};

static USAGE: &'static str = "
Usage:
    check-container-ram [--show-hogs=<count>] [--invalid-limit=<status>] [options]
    check-container-ram (-h | --help)

Check the RAM usage of the currently-running container. This must be run from
inside the container to be checked.

This checks as a ratio of the limit specified in the cgroup memory limit, and
if there is no limit set (or the limit is greater than the total memory
available on the system) this checks against the total system memory.

Options:
    -h, --help                 Show this message and exit

    -w, --warn=<percent>       Warn at this percent used           [default: 85]
    -c, --crit=<percent>       Critical at this percent used       [default: 95]

    --invalid-limit=<status>   Status to consider this check if the CGroup limit
                               is greater than the system ram      [default: ok]

    --show-hogs=<count>        Show the most ram-hungry procs      [default: 0]
";

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_warn: f64,
    flag_crit: f64,

    flag_invalid_limit: Status,
    flag_show_hogs: u32,
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
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());
    let mut status = Status::Ok;

    let mut limit = limit_in_bytes().unwrap();
    let mut limit_type = Limit::CGroup;
    let mem = MemInfo::load();
    let system_bytes = mem.total.unwrap() * 1024;
    if limit > system_bytes {
        if args.flag_invalid_limit != Status::Ok {
            println!(
                "{}: CGroup memory limit is greater than system memory ({} > {})",
                args.flag_invalid_limit,
                bytes_to_human_size(limit as u64),
                bytes_to_human_size(system_bytes as u64)
            );
            status = args.flag_invalid_limit;
        }
        limit = system_bytes;
        limit_type = Limit::System;
    }

    let cgroup_stat = Stat::load().unwrap();
    let ratio = cgroup_stat.rss as f64 / limit as f64;
    let percent = ratio * 100.0;

    if percent > args.flag_crit {
        println!(
            "CRITICAL: cgroup is using {:.1}% of {} {} (greater than {}%)",
            percent,
            bytes_to_human_size(limit as u64),
            limit_type,
            args.flag_crit
        );
        status = Status::Critical;
    } else if percent > args.flag_warn {
        println!(
            "WARNING: cgroup is using {:.1}% of {} {} (greater than {}%)",
            percent,
            bytes_to_human_size(limit as u64),
            limit_type,
            args.flag_warn
        );
        status = max(status, Status::Warning);
    } else {
        println!(
            "OK: cgroup is using {:.1}% of {} {} (less than {}%)",
            percent,
            bytes_to_human_size(limit as u64),
            limit_type,
            args.flag_warn
        );
    }

    if args.flag_show_hogs > 0 {
        let per_proc = RunningProcs::currently_running().unwrap();
        let mut procs = per_proc.0.values().collect::<Vec<_>>();
        procs.sort_by(|l, r| r.stat.rss.cmp(&l.stat.rss));
        println!("INFO [check-container-ram]: ram hogs");
        for process in procs.iter().take(args.flag_show_hogs as usize) {
            let percent = process.percent_ram(limit);
            println!(
                "[{:>6}]{:>5.1}% {:>6}: {}",
                process.stat.pid,
                percent,
                pages_to_human_size(process.stat.rss),
                process.useful_cmdline()
            );
        }
    }
    status.exit();
}

#[cfg(test)]
mod unit {

    use docopt::Docopt;
    use tabin_plugins::Status;
    use super::{Args, USAGE};

    #[test]
    fn usage_is_valid() {
        let args: Args = Docopt::new(USAGE).and_then(|d| d.decode()).unwrap();
        assert_eq!(args.flag_crit, 95.0);
        assert_eq!(args.flag_invalid_limit, Status::Ok);
        let args: Args = Docopt::new(USAGE)
            .and_then(|d| {
                d.argv(vec!["arg0", "--crit", "80", "--warn", "20"].into_iter())
                    .decode()
            })
            .unwrap();
        assert_eq!(args.flag_crit, 80.0);
    }
}

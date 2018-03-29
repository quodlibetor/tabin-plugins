#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate structopt;
extern crate tabin_plugins;

use structopt::StructOpt;

use tabin_plugins::linux::pages_to_human_size;
use tabin_plugins::Status;
use tabin_plugins::procfs::{LoadProcsError, MemInfo, ProcFsError, RunningProcs};

/// Check the ram usage of the current computer
#[derive(Deserialize, StructOpt, Debug)]
#[structopt(name = "check-ram (part of tabin-plugins)",
            raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
struct Args {
    #[structopt(short = "w", long = "warn", help = "Percent to warn at", default_value = "85")]
    warn: f64,
    #[structopt(short = "c", long = "crit", help = "Percent to go critical at",
                default_value = "95")]
    crit: f64,

    #[structopt(long = "show-hogs", name = "count",
                help = "Show <count> most ram-intensive processes in this computer.",
                default_value = "0")]
    show_hogs: usize,
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args = Args::from_args();
    let mem = MemInfo::load();
    let status = compare_status(args.crit, args.warn, &mem);
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
            "INFO [check-ram]: {} processes running, top {} ram hogs:",
            procs.len(), args.show_hogs
        );
        for process in procs.iter().take(args.show_hogs) {
            let system_kb = mem.total.unwrap();
            let percent = process.percent_ram(system_kb * 1024);
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
    };
    status.exit();
}

fn compare_status(crit: f64, warn: f64, mem: &MemInfo) -> Status {
    match mem.percent_used() {
        Ok(percent) => if percent > crit {
            println!("CRITICAL [check-ram]: {:.1}% > {}%", percent, crit);
            Status::Critical
        } else if percent > warn {
            println!("WARNING [check-ram]: {:.1}% > {}%", percent, warn);
            Status::Warning
        } else {
            println!("OK [check-ram]: {:.1}% < {}%", percent, warn);
            Status::Ok
        },
        Err(e) => {
            println!("UNKNOWN [check-ram]: UNEXPECTED ERROR {:?}", e);
            Status::Unknown
        }
    }
}

#[cfg(test)]
mod test {
    use super::compare_status;
    use tabin_plugins::Status;
    use tabin_plugins::procfs::MemInfo;

    #[test]
    fn alerts_when_told_to() {
        let mem = MemInfo {
            total: Some(100),
            available: Some(15), // 15% free means 85% used
            free: None,
            cached: None,
        };
        let crit_threshold = 80.0;

        assert_eq!(compare_status(crit_threshold, 25.0, &mem), Status::Critical);
    }
}

extern crate rustc_serialize;

extern crate docopt;
extern crate tabin_plugins;

use docopt::Docopt;

use tabin_plugins::linux::pages_to_human_size;
use tabin_plugins::Status;
use tabin_plugins::procfs::{LoadProcsError, MemInfo, ProcFsError, RunningProcs};

static USAGE: &'static str = "
Usage: check-ram [options]
       check-ram -h | --help

Options:
    -h, --help             Show this help message

    -w, --warn=<percent>   Percent used to warn at      [default: 80]
    -c, --crit=<percent>   Percent used to critical at  [default: 95]

    --show-hogs=<count>    Show most RAM-hungry procs   [default: 0]
";

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_help: bool,
    flag_warn: f64,
    flag_crit: f64,
    flag_show_hogs: usize,
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

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());
    let mem = MemInfo::load();
    let status = compare_status(args.flag_crit, args.flag_warn, &mem);
    if args.flag_show_hogs > 0 {
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
        println!("INFO [check-ram]: ram hogs");
        for process in procs.iter().take(args.flag_show_hogs) {
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

#[cfg(test)]
mod test {
    use docopt::Docopt;
    use super::{compare_status, Args, USAGE};
    use tabin_plugins::Status;
    use tabin_plugins::procfs::MemInfo;

    #[test]
    fn usage_is_valid() {
        let _: Args = Docopt::new(USAGE).and_then(|d| d.decode()).unwrap();
    }

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

extern crate rustc_serialize;

extern crate docopt;
extern crate turbine_plugins;

use docopt::Docopt;

use turbine_plugins::Status;
use turbine_plugins::procfs::{RunningProcs, MemInfo};

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
    flag_warn:  f64,
    flag_crit:  f64,
    flag_show_hogs: usize,
}

fn compare_status(crit: f64, warn: f64, mem: &MemInfo) -> Status {
    match mem.percent_used() {
        Ok(percent) => {
            if percent > crit {
                println!("CRITICAL [check-ram]: {:.1}% > {}%", percent, crit);
                Status::Critical
            } else if percent > warn {
                println!("WARNING [check-ram]: {:.1}% > {}%", percent, warn);
                Status::Warning
            } else {
                println!("OK [check-ram]: {:.1}% < {}%", percent, warn);
                Status::Ok
            }
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
        let per_proc = RunningProcs::currently_running().unwrap();
        let mut procs = per_proc.0.values().collect::<Vec<_>>();
        procs.sort_by(|l, r| r.stat.rss.cmp(&l.stat.rss));
        if args.flag_show_hogs > 0 {
            println!("INFO [check-ram]: ram hogs");
            for process in procs.iter().take(args.flag_show_hogs) {
                let percent = process.percent_ram(&mem).unwrap_or(0.0);
                println!("     {:.1}%: {}", percent, process.useful_cmdline());
            }
        }
    };
    status.exit();
}

#[cfg(test)]
mod test {
    use docopt::Docopt;
    use super::{USAGE, Args, compare_status};
    use turbine_plugins::Status;
    use turbine_plugins::procfs::MemInfo;

    #[test]
    fn usage_is_valid() {
        let _: Args = Docopt::new(USAGE).and_then(|d| d.decode()).unwrap();
    }

    #[test]
    fn alerts_when_told_to() {
        let mem = MemInfo {
            total: Some(100),
            available: Some(15),  // 15% free means 85% used
            free: None,
            cached: None
        };
        let crit_threshold = 80.0;

        assert_eq!(compare_status(crit_threshold, 25.0, &mem), Status::Critical);
    }
}

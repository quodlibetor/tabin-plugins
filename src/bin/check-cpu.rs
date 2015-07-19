//! Check CPU usage

extern crate rustc_serialize;

extern crate docopt;

extern crate turbine_plugins;

use docopt::Docopt;
use turbine_plugins::ExitStatus;
use turbine_plugins::procfs::{Calculations, RunningProcs, WorkSource};

static USAGE: &'static str = "
Usage: check-cpu [options] [--type=<work-source>...] [--show-hogs=<count>]

Options:
    -h, --help               Show this help message

    -s, --sample=<seconds>   Seconds to spent collecting   [default: 1]
    -w, --warn=<percent>     Percent to warn at            [default: 80]
    -c, --crit=<percent>     Percent to critical at        [default: 95]
    --show-hogs=<count>      Show most cpu-hungry procs    [default: 0]

CPU Work Types:

    Specifying one of the CPU kinds checks that kind of utilization. The
    default is to check total utilization.

    --type=<usage>           Some of:
                                total user nice system idle
                                iowait irq softirq steal guest [default: total]
";

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_help: bool,
    flag_sample: i32,
    flag_warn: isize,
    flag_crit: isize,
    flag_show_hogs: usize,

    flag_type: Vec<WorkSource>,
}

fn do_comparison(args: &Args, start: &Calculations, end: &Calculations) -> ExitStatus {
    let mut exit_status = ExitStatus::Ok;

    for flag in &args.flag_type {
        let total = end.percent_util_since(flag, &start);
        if total > args.flag_crit as f64 {
            exit_status = std::cmp::max(exit_status, ExitStatus::Critical);
            println!("CRITICAL [check-cpu]: {} {} > {}", flag, total, args.flag_crit);
        } else if total > args.flag_warn as f64 {
            exit_status = std::cmp::max(exit_status, ExitStatus::Warning);
            println!("WARNING [check-cpu]: {} {} > {}", flag, total, args.flag_warn);
        } else {
            println!("OK [check-cpu]");
        }
    }

    exit_status
}


#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());
    if args.flag_help {
        print!("{}", USAGE);
        return;
    }

    let start = Calculations::load();
    let start_per_proc = RunningProcs::currently_running().unwrap();
    std::thread::sleep_ms((args.flag_sample * 1000) as u32);
    let end_per_proc = RunningProcs::currently_running().unwrap();
    let end = Calculations::load();
    let mut per_proc = end_per_proc.percent_cpu_util_since(&start_per_proc,
                                                            end.total() - start.total());
    per_proc.0.sort_by(|l, r| r.total.partial_cmp(&l.total).unwrap());
    for usage in per_proc.0.iter().take(args.flag_show_hogs) {
        println!("{}: {:.1}%", usage.proc_stat.comm, usage.total);
    }
    let status = do_comparison(&args, &start, &end);
    status.exit()
}

#[cfg(test)]
mod unit {
    use docopt::Docopt;
    use super::{USAGE};

    use turbine_plugins::procfs::{Calculations, WorkSource};

    #[test]
    fn validate_docstring() {
        Docopt::new(USAGE).unwrap();
    }

    #[test]
    fn validate_allows_multiple_worksources() {
        let argv = || vec!["check-cpu", "--type", "total", "--type", "steal"];
        let _: super::Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(argv().into_iter()).decode())
            .unwrap();
    }

    fn begintime() -> Calculations {
        Calculations {
            user: 100.0,
            nice: 100.0,
            system: 100.0,
            idle: 100.0,
            iowait: 100.0,
            irq: 100.0,
            softirq: 100.0,
            steal: 100.0,
            guest: 100.0,
            guest_nice: Some(0.0),
        }
    }

    #[test]
    fn percentage_user_idle() {
        let start = begintime();

        let end = Calculations {
            user: 110.0,
            idle: 110.0,
            ..start
        };

        assert_eq!(start.percent_util_since(&WorkSource::User, &end), 50.0)
    }

    #[test]
    fn percentage_user() {
        let start = begintime();

        let end = Calculations {
            user: 110.0,
            ..start
        };

        assert_eq!(start.percent_util_since(&WorkSource::User, &end), 100.0)
    }

    #[test]
    fn percentage_total_user_idle() {
        let start = begintime();
        let end = Calculations {
            user: 110.0,
            idle: 110.0,
            ..start
        };

        assert_eq!(start.percent_util_since(&WorkSource::Total, &end), 50.0)
    }

    #[test]
    fn percentage_total_user_idle_system_steal() {
        let start = begintime();
        let end = Calculations {
            user: 110.0,
            system: 110.0,
            steal: 110.0,
            idle: 110.0,
            ..start
        };

        assert_eq!(start.percent_util_since(&WorkSource::Total, &end), 75.0)
    }
}

#[cfg(test)]
mod integration {
    // not really integration tests, but higher level
    use super::{do_comparison, USAGE};

    use turbine_plugins::ExitStatus;
    use turbine_plugins::procfs::Calculations;
    use docopt::Docopt;

    fn start() -> Calculations {
        Calculations {
            user: 100.0,
            nice: 100.0,
            system: 100.0,
            idle: 100.0,
            iowait: 100.0,
            irq: 100.0,
            softirq: 100.0,
            steal: 100.0,
            guest: 100.0,
            guest_nice: Some(0.0),
        }
    }

    #[test]
    fn does_alert() {
        let argv = || vec!["check-cpu", "-c", "49", "--type", "total", "--type", "steal"];
        let start = start();
        let end = Calculations {
            user: 110.0,
            idle: 110.0,
            ..start
        };
        let args: super::Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(argv().into_iter()).decode())
            .unwrap();

        assert_eq!(do_comparison(&args, &start, &end),
                   ExitStatus::Critical);
    }
}

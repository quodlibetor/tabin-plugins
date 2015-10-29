//! Check CPU usage

extern crate rustc_serialize;

extern crate docopt;

extern crate turbine_plugins;
// extern crate rand;

use std::fmt::Display;
use std::cmp::{PartialOrd, max};

use docopt::Docopt;
use turbine_plugins::Status;
use turbine_plugins::procfs::{Calculations, RunningProcs, WorkSource};
use turbine_plugins::sys::fs::cgroup::cpuacct::Stat as CGroupStat;
use turbine_plugins::linux::Ratio;

static USAGE: &'static str = "
Usage:
    check-cpu [options] [--type=<work-source>...] [--container] [--show-hogs=<count>]
    check-cpu (-h | --help)

Options:
    -h, --help               Show this help message

    -s, --sample=<seconds>   Seconds to spent collecting   [default: 1]
    -w, --warn=<percent>     Percent to warn at            [default: 80]
    -c, --crit=<percent>     Percent to critical at        [default: 95]
    --show-hogs=<count>      Show most cpu-hungry procs    [default: 0]

CPU Work Types:

    Specifying one of the CPU kinds checks that kind of utilization. The
    default is to check total utilization. Specifying this multiple times
    alerts if *any* of the CPU usage types are critical.

    There are three CPU type groups: `active` `activeplusiowait` and
    `activeminusnice`. `activeplusiowait` considers time spent waiting for IO
    to be busy time, this gets alerts to be more aligned with the overall
    system load, but is different from CPU usage reported by `top` since the
    CPU isn't actually *busy* during this time.

    --type=<usage>           Some of:
                                active activeplusiowait activeminusnice
                                user nice system irq softirq steal guest
                                idle iowait [default: active]

    --container              Use the total process from inside the currently
                             running container

";

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_help: bool,
    flag_sample: u32,
    flag_warn: f64,
    flag_crit: f64,
    flag_show_hogs: usize,

    flag_type: Vec<WorkSource>,
    flag_container: bool,
}

fn print_errors_and_status<T: Display, V: PartialOrd<V> + Display>(
    flag: T, total: V, critical: V, warning: V, mut exit_status: Status) -> Status {
    if total > critical {
        exit_status = std::cmp::max(exit_status, Status::Critical);
        println!("CRITICAL [check-cpu]: {} {:.2} > {}%", flag, total, critical);
    } else if total > warning {
        exit_status = std::cmp::max(exit_status, Status::Warning);
        println!("WARNING [check-cpu]: {} {:.2} > {}%", flag, total, warning);
    } else {
        println!("OK [check-cpu]: {} {:.2} < {}%", flag, total, warning);
    }
    exit_status
}

/// Check if we have exceeded the warn or critical limits
///
/// The *cstat parameters, if present, mean that in addition to performing the
/// standard checks we check the currently-running container against the same
/// thresholds.
fn do_comparison<'a>(args: &Args,
                     start: &'a Calculations,
                     end: &'a Calculations,
                     start_cstat: &'a Option<CGroupStat>,
                     end_cstat: &'a Option<CGroupStat>
) -> Status {
    let mut exit_status = Status::Ok;

    for flag in &args.flag_type {
        let total = end.percent_util_since(flag, &start);
        exit_status = std::cmp::max(
            exit_status,
            print_errors_and_status(
                flag, total, args.flag_crit.into(), args.flag_warn, exit_status));
    }
    if let (&Some(ref cstart), &Some(ref cend)) = (start_cstat, end_cstat) {
        let total = (end.total() - start.total()).duration();
        let container = (cend.total() - cstart.total()).duration();
        let usage_ratio = container.ratio(&total);
        exit_status = std::cmp::max(
            exit_status,
            print_errors_and_status(
                "container", usage_ratio, args.flag_crit, args.flag_warn, exit_status));
    }
    println!("INFO [check-cpu]: Usage breakdown: {}", end - start);

    exit_status
}


#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    let mut cstat_start = None;
    let mut cstat_end = None;
    let mut status = Status::Ok;

    if args.flag_container {
        match CGroupStat::load() {
            Ok(val) => cstat_start = Some(val),
            Err(e) => {
                println!(
                    "WARNING [check-cpu]: unable to load container stats from /sys/fs (are you running from inside a container?): {}",
                    e);
                status = max(status, Status::Warning);
            }
        };
    }
    let start = Calculations::load().unwrap();
    let start_per_proc = RunningProcs::currently_running().unwrap();

    std::thread::sleep_ms(args.flag_sample * 1000);

    let end_per_proc = RunningProcs::currently_running().unwrap();
    let end = Calculations::load().unwrap();
    let mut per_proc = end_per_proc.percent_cpu_util_since(&start_per_proc,
                                                           end.total() - start.total());
    per_proc.0.sort_by(|l, r| r.total.partial_cmp(&l.total).unwrap());

    if args.flag_container {
        cstat_end = CGroupStat::load().ok()
    }

    status = max(status, do_comparison(&args, &start, &end, &cstat_start, &cstat_end));

    if args.flag_show_hogs > 0 {
        println!("INFO [check-cpu]: hogs");
        for usage in per_proc.0.iter().take(args.flag_show_hogs) {
            println!("     {:.1}%: {}",
                     usage.total,
                     usage.process.useful_cmdline());
        }
    }
    status.exit()
}

#[cfg(test)]
mod unit {
    use docopt::Docopt;
    use super::{USAGE};

    use turbine_plugins::procfs::{Calculations, WorkSource};
    use turbine_plugins::linux::Jiffies;

    #[test]
    fn validate_docstring() {
        Docopt::new(USAGE).unwrap();
    }

    #[test]
    fn validate_allows_multiple_worksources() {
        let argv = || vec!["check-cpu", "--type", "active", "--type", "steal"];
        let _: super::Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(argv().into_iter()).decode())
            .unwrap();
    }

    fn begintime() -> Calculations {
        Calculations {
            user: Jiffies::new(100),
            nice: Jiffies::new(100),
            system: Jiffies::new(100),
            idle: Jiffies::new(100),
            iowait: Jiffies::new(100),
            irq: Jiffies::new(100),
            softirq: Jiffies::new(100),
            steal: Jiffies::new(100),
            guest: Jiffies::new(100),
            guest_nice: Some(Jiffies::new(0)),
        }
    }

    #[test]
    fn percentage_user_idle() {
        let start = begintime();

        let end = Calculations {
            user: Jiffies::new(110),
            idle: Jiffies::new(110),
            ..start
        };

        assert_eq!(end.percent_util_since(&WorkSource::User, &start), 50.0)
    }

    #[test]
    fn percentage_user() {
        let start = begintime();

        let end = Calculations {
            user: Jiffies::new(110),
            ..start
        };

        assert_eq!(end.percent_util_since(&WorkSource::User, &start), 100.0)
    }

    #[test]
    fn percentage_total_user_idle() {
        let start = begintime();
        let end = Calculations {
            user: Jiffies::new(110),
            idle: Jiffies::new(110),
            ..start
        };

        assert_eq!(end.percent_util_since(&WorkSource::Active, &start), 50.0)
    }

    #[test]
    fn percentage_total_user_idle_system_steal() {
        let start = begintime();
        let end = Calculations {
            user: Jiffies::new(110),
            system: Jiffies::new(110),
            steal: Jiffies::new(110),
            idle: Jiffies::new(110),
            ..start
        };

        assert_eq!(end.percent_util_since(&WorkSource::Active, &start), 75.0)
    }
}

#[cfg(target_os = "linux")]
#[cfg(test)]
mod integration {
    // not really integration tests, but higher level
    use super::{do_comparison, USAGE};

    use turbine_plugins::Status;
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
        let argv = || vec!["check-cpu", "-c", "49", "--type", "active", "--type", "steal"];
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
                   Status::Critical);
    }
}

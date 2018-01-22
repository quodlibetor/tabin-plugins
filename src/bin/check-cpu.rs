//! Check CPU usage

extern crate rustc_serialize;

extern crate docopt;

extern crate tabin_plugins;
// extern crate rand;

use std::fmt::Display;
use std::cmp::PartialOrd;
use std::thread::sleep;
use std::time::Duration;

use docopt::Docopt;
use tabin_plugins::Status;
use tabin_plugins::procfs::{Calculations, RunningProcs, WorkSource};

static USAGE: &'static str = "
Usage:
    check-cpu [options] [--type=<work-source>...] [--show-hogs=<count>]
    check-cpu (-h | --help)

Options:
    -h, --help               Show this help message

    -s, --sample=<seconds>   Seconds to spent collecting   [default: 1]
    -w, --warn=<percent>     Percent to warn at            [default: 80]
    -c, --crit=<percent>     Percent to critical at        [default: 95]

    --per-cpu                Gauge values per-cpu instead of across the
                             entire machine
    --cpu-count=<num>        If --per-cpu is specified, this is how many
                             CPUs need to be at a threshold to trigger.
                             [default: 1]

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
";

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_help: bool,
    flag_sample: u32,
    flag_warn: f64,
    flag_crit: f64,
    flag_show_hogs: usize,

    flag_per_cpu: bool,
    flag_cpu_count: u32,

    flag_type: Vec<WorkSource>,
}

fn print_errors_and_status<T: Display, V: PartialOrd<V> + Display>(
    flag: T,
    total: V,
    critical: V,
    warning: V,
    mut exit_status: Status,
) -> Status {
    if total > critical {
        exit_status = std::cmp::max(exit_status, Status::Critical);
        println!(
            "CRITICAL [check-cpu]: {} {:.2} > {}%",
            flag, total, critical,
        );
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
fn do_comparison<'a>(args: &Args, start: &'a Calculations, end: &'a Calculations) -> Status {
    let mut exit_status = Status::Ok;

    for flag in &args.flag_type {
        let total = end.percent_util_since(flag, &start);
        exit_status = std::cmp::max(
            exit_status,
            print_errors_and_status(
                flag,
                total,
                args.flag_crit.into(),
                args.flag_warn,
                exit_status,
            ),
        );
    }
    println!("INFO [check-cpu]: Usage breakdown: {}", end - start);

    exit_status
}

fn determine_status_per_cpu(
    args: &Args,
    start: &[Calculations],
    end: &[Calculations],
) -> Vec<Status> {
    start
        .iter()
        .enumerate()
        .map(|(i, val)| do_comparison(&args, &val, &end[i]))
        .collect::<Vec<_>>()
}

fn determine_exit(args: &Args, statuses: &[Status]) -> Status {
    let crit = statuses
        .iter()
        .filter(|status| **status == Status::Critical)
        .collect::<Vec<_>>();
    let warn = statuses
        .iter()
        .filter(|status| **status == Status::Warning)
        .collect::<Vec<_>>();
    if crit.len() >= args.flag_cpu_count as usize {
        Status::Critical
    } else if warn.len() >= args.flag_cpu_count as usize {
        Status::Warning
    } else {
        Status::Ok
    }
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    let start = if args.flag_per_cpu {
        Calculations::load_per_cpu().unwrap()
    } else {
        vec![Calculations::load().unwrap()]
    };
    let mut start_per_proc = None;
    if args.flag_show_hogs > 0 {
        start_per_proc = Some(RunningProcs::currently_running().unwrap());
    }
    sleep(Duration::from_millis(args.flag_sample as u64 * 1000));

    let end = if args.flag_per_cpu {
        Calculations::load_per_cpu().unwrap()
    } else {
        vec![Calculations::load().unwrap()]
    };
    let statuses = determine_status_per_cpu(&args, &start, &end);

    if args.flag_show_hogs > 0 {
        let end_per_proc = RunningProcs::currently_running().unwrap();
        let start_per_proc = start_per_proc.unwrap();
        let single_start = &start[0];
        let single_end = &end[0];
        let mut per_proc = end_per_proc
            .percent_cpu_util_since(&start_per_proc, single_end.total() - single_start.total());
        per_proc
            .0
            .sort_by(|l, r| r.total.partial_cmp(&l.total).unwrap());
        println!("INFO [check-cpu]: hogs");
        for usage in per_proc.0.iter().take(args.flag_show_hogs) {
            println!(
                "[{:>5}]{:>5.1}%: {}",
                usage.process.stat.pid,
                usage.total,
                usage.process.useful_cmdline()
            );
        }
    }

    determine_exit(&args, &statuses).exit()
}

#[cfg(test)]
mod unit {
    use docopt::Docopt;
    use super::{determine_exit, determine_status_per_cpu, do_comparison, Args, USAGE};

    use tabin_plugins::Status;
    use tabin_plugins::procfs::{Calculations, WorkSource};
    use tabin_plugins::linux::Jiffies;

    #[test]
    fn validate_docstring() {
        let _: Args = Docopt::new(USAGE)
            .and_then(|d| {
                d.argv(vec!["arg0", "--per-cpu"].into_iter())
                    .help(true)
                    .decode()
            })
            .unwrap();
        let args: Args = Docopt::new(USAGE)
            .and_then(|d| {
                d.argv(vec!["arg0", "--per-cpu", "--cpu-count", "2"].into_iter())
                    .decode()
            })
            .unwrap();
        assert_eq!(args.flag_per_cpu, true);
        let args: Args = Docopt::new(USAGE)
            .and_then(|d| {
                d.argv(vec!["arg0", "--show-hogs", "5"].into_iter())
                    .decode()
            })
            .unwrap();
        assert_eq!(args.flag_per_cpu, false);
        assert_eq!(args.flag_show_hogs, 5);
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

    fn start() -> Calculations {
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
    fn does_alert() {
        let argv = || {
            vec![
                "check-cpu",
                "-c",
                "49",
                "--type",
                "active",
                "--type",
                "steal",
            ]
        };
        let start = start();
        let end = Calculations {
            user: Jiffies::new(110),
            idle: Jiffies::new(110),
            ..start
        };
        let args: super::Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(argv().into_iter()).decode())
            .unwrap();

        assert_eq!(do_comparison(&args, &start, &end), Status::Critical);
    }

    // Exactly the same as does_alert, but also validate the determine* functions
    #[test]
    fn does_alert_per_cpu() {
        let argv = || {
            vec![
                "check-cpu",
                "-c",
                "49",
                "--type",
                "active",
                "--type",
                "steal",
            ]
        };
        let start = vec![start()];
        let end = vec![
            Calculations {
                user: Jiffies::new(110),
                idle: Jiffies::new(110),
                ..start[0]
            },
        ];
        let args: super::Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(argv().into_iter()).decode())
            .unwrap();
        let statuses = determine_status_per_cpu(&args, &start, &end);
        assert_eq!(statuses, vec![Status::Critical]);
        assert_eq!(determine_exit(&args, &statuses), Status::Critical);
    }

    #[test]
    fn does_alert_per_cpu_with_some_ok() {
        let argv = || {
            vec![
                "check-cpu",
                "-c",
                "49",
                "--type",
                "active",
                "--per-cpu",
                "--cpu-count",
                "2",
            ]
        };
        let start = vec![start(), start(), start()];
        let mut end = vec![
            Calculations {
                user: Jiffies::new(110),
                idle: Jiffies::new(110),
                ..start[0]
            },
            Calculations {
                user: Jiffies::new(105),
                idle: Jiffies::new(110),
                ..start[0]
            },
            Calculations {
                user: Jiffies::new(105),
                idle: Jiffies::new(110),
                ..start[0]
            },
        ];
        let args: super::Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(argv().into_iter()).decode())
            .unwrap();
        let statuses = determine_status_per_cpu(&args, &start, &end);
        assert_eq!(statuses, vec![Status::Critical, Status::Ok, Status::Ok]);
        assert_eq!(determine_exit(&args, &statuses), Status::Ok);

        end[1] = Calculations {
            user: Jiffies::new(110),
            idle: Jiffies::new(110),
            ..start[0]
        };
        let args: super::Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(argv().into_iter()).decode())
            .unwrap();
        let statuses = determine_status_per_cpu(&args, &start, &end);
        assert_eq!(
            statuses,
            vec![Status::Critical, Status::Critical, Status::Ok]
        );
        assert_eq!(determine_exit(&args, &statuses), Status::Critical);
    }
}

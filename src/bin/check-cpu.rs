//! Check CPU usage

extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

extern crate tabin_plugins;

use std::fmt::Display;
use std::cmp::PartialOrd;
use std::thread::sleep;
use std::time::Duration;
use structopt::StructOpt;

use tabin_plugins::Status;
use tabin_plugins::procfs::{Calculations, LoadProcsError, ProcField, ProcFsError, RunningProcs,
                            WorkSource};

#[derive(Deserialize, StructOpt, Debug)]
#[structopt(name = "check-cpu  (part of tabin-plugins)",
            raw(setting = "structopt::clap::AppSettings::ColoredHelp"),
            after_help = "CPU Work Types:

    Specifying one of the CPU kinds via `--type` checks that kind of
    utilization. The default is to check total utilization. Specifying this
    multiple times alerts if *any* of the CPU usage types are critical.

    There are three CPU type groups: `active` `activeplusiowait` and
    `activeminusnice`. `activeplusiowait` considers time spent waiting for IO
    to be busy time, this gets alerts to be more aligned with the overall
    system load, but is different from CPU usage reported by `top` since the
    CPU isn't actually *busy* during this time.

    --type=<usage>           Some of:
                                active activeplusiowait activeminusnice
                                user nice system irq softirq steal guest
                                idle iowait [default: active]")]
struct Args {
    #[structopt(short = "w", long = "warn", help = "Percent to warn at", default_value = "80")]
    warn: f64,
    #[structopt(short = "c", long = "crit", help = "Percent to go critical at",
                default_value = "80")]
    crit: f64,
    #[structopt(short = "s", long = "sample", name = "seconds",
                help = "Seconds to take sample over", default_value = "5")]
    sample: u64,
    #[structopt(long = "show-hogs", name = "count",
                help = "Show <count> most cpu-intensive processes in this container.",
                default_value = "0")]
    show_hogs: usize,

    #[structopt(long = "per-cpu",
                help = "Gauge values per-cpu instead of across the entire machine")]
    per_cpu: bool,
    #[structopt(long = "cpu-count", default_value = "1",
                help = "If --per-cpu is specified, this is how many
                             CPUs need to be at a threshold to trigger.")]
    cpu_count: u32,

    #[structopt(long = "type", default_value = "active", help = "See 'CPU Work Types, below")]
    work_type: Vec<WorkSource>,
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

    for flag in &args.work_type {
        let total = end.percent_util_since(flag, &start);
        exit_status = std::cmp::max(
            exit_status,
            print_errors_and_status(flag, total, args.crit.into(), args.warn, exit_status),
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
    if crit.len() >= args.cpu_count as usize {
        Status::Critical
    } else if warn.len() >= args.cpu_count as usize {
        Status::Warning
    } else {
        Status::Ok
    }
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args: Args = Args::from_args();

    let start = if args.per_cpu {
        Calculations::load_per_cpu().unwrap()
    } else {
        vec![Calculations::load().unwrap()]
    };
    let mut load_errors = vec![];
    let mut start_per_proc = None;
    if args.show_hogs > 0 {
        start_per_proc = Some(load_procs(&mut load_errors));
    }
    sleep(Duration::from_millis(args.sample as u64 * 1000));

    let end = if args.per_cpu {
        Calculations::load_per_cpu().unwrap()
    } else {
        vec![Calculations::load().unwrap()]
    };
    let statuses = determine_status_per_cpu(&args, &start, &end);

    if args.show_hogs > 0 {
        let end_per_proc = load_procs(&mut load_errors);
        let start_per_proc = start_per_proc.unwrap();
        let single_start = &start[0];
        let single_end = &end[0];
        let mut per_proc = end_per_proc
            .percent_cpu_util_since(&start_per_proc, single_end.total() - single_start.total());
        per_proc.sort_by_field(ProcField::TotalCpu);
        println!(
            "INFO [check-cpu]: {} processes running, top {} cpu hogs:",
            per_proc.len(),
            args.show_hogs
        );
        for usage in per_proc.iter().take(args.show_hogs) {
            println!(
                "[{:>5}]{:>5.1}%: {}",
                usage.process.stat.pid,
                usage.total,
                usage.process.useful_cmdline()
            );
        }
        if !load_errors.is_empty() {
            eprintln!("Error loading some per-process information:");
            for error in &load_errors {
                eprintln!("    {}", error);
            }
        }
    }

    determine_exit(&args, &statuses).exit()
}

/// Load currently running procs, and die if there is a surprising error
fn load_procs(load_errors: &mut Vec<ProcFsError>) -> RunningProcs {
    match RunningProcs::currently_running() {
        Ok(procs) => procs,
        Err(ProcFsError::LoadProcsError(LoadProcsError { procs, errors })) => {
            load_errors.extend(errors.into_iter());
            procs
        }
        Err(err) => {
            eprintln!("Unexpected error loading procs: {}", err);
            Status::Unknown.exit()
        }
    }
}

#[cfg(test)]
mod unit {
    use super::{determine_exit, determine_status_per_cpu, do_comparison, Args};

    use structopt::StructOpt;

    use tabin_plugins::Status;
    use tabin_plugins::procfs::{Calculations, WorkSource};
    use tabin_plugins::linux::Jiffies;

    #[test]
    fn validate_docstring() {
        let _: Args = Args::from_iter(["arg0", "--per-cpu"].into_iter());
        let args: Args = Args::from_iter(["arg0", "--per-cpu", "--cpu-count", "2"].into_iter());
        assert_eq!(args.per_cpu, true);
        let args: Args = Args::from_iter(["arg0", "--show-hogs", "5"].into_iter());
        assert_eq!(args.per_cpu, false);
        assert_eq!(args.show_hogs, 5);
    }

    #[test]
    fn validate_allows_multiple_worksources() {
        Args::from_iter(["check-cpu", "--type", "active", "--type", "steal"].into_iter());
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
        let start = start();
        let end = Calculations {
            user: Jiffies::new(110),
            idle: Jiffies::new(110),
            ..start
        };
        let args: super::Args = Args::from_iter(
            [
                "check-cpu",
                "-c",
                "49",
                "--type",
                "active",
                "--type",
                "steal",
            ].into_iter(),
        );

        assert_eq!(do_comparison(&args, &start, &end), Status::Critical);
    }

    // Exactly the same as does_alert, but also validate the determine* functions
    #[test]
    fn does_alert_per_cpu() {
        let start = vec![start()];
        let end = vec![
            Calculations {
                user: Jiffies::new(110),
                idle: Jiffies::new(110),
                ..start[0]
            },
        ];
        let args: super::Args = Args::from_iter(
            [
                "check-cpu",
                "-c",
                "49",
                "--type",
                "active",
                "--type",
                "steal",
            ].into_iter(),
        );
        assert_eq!(args.work_type, vec![WorkSource::Active, WorkSource::Steal]);
        let statuses = determine_status_per_cpu(&args, &start, &end);
        assert_eq!(statuses, vec![Status::Critical]);
        assert_eq!(determine_exit(&args, &statuses), Status::Critical);
    }

    #[test]
    fn does_alert_per_cpu_with_some_ok() {
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
        let argv = || {
            [
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
        let args: super::Args = Args::from_iter(argv().into_iter());
        let statuses = determine_status_per_cpu(&args, &start, &end);
        assert_eq!(statuses, vec![Status::Critical, Status::Ok, Status::Ok]);
        assert_eq!(determine_exit(&args, &statuses), Status::Ok);

        end[1] = Calculations {
            user: Jiffies::new(110),
            idle: Jiffies::new(110),
            ..start[0]
        };
        let args: super::Args = Args::from_iter(argv().into_iter());
        let statuses = determine_status_per_cpu(&args, &start, &end);
        assert_eq!(
            statuses,
            vec![Status::Critical, Status::Critical, Status::Ok]
        );
        assert_eq!(determine_exit(&args, &statuses), Status::Critical);
    }
}

//! Check the load average of the system
//!
//! Load average is the number of processes *waiting* to do work in a queue, either
//! due to IO or CPU constraints. The numbers used to check are the load averaged
//! over 1, 5 and 15 minutes, respectively

use serde::Deserialize;
use structopt::StructOpt;

use tabin_plugins::procfs::{Calculations, LoadAvg};
use tabin_plugins::Status;

/// Check the load average of the system
///
/// Load average is the number of processes *waiting* to do work in a queue, either
/// due to IO or CPU constraints. The numbers used to check are the load averaged
/// over 1, 5 and 15 minutes, respectively
#[derive(Deserialize, Debug, StructOpt)]
#[structopt(
    name = "check-load (part of tabin-plugins)",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct Args {
    #[structopt(
        short = "w",
        long = "warn",
        help = "Averages to warn at",
        default_value = "5,3.5,2.5"
    )]
    warn: LoadAvg,
    #[structopt(
        short = "c",
        long = "crit",
        help = "Averages to go critical at",
        default_value = "10,5,3"
    )]
    crit: LoadAvg,
    #[structopt(
        long = "per-cpu",
        help = "Divide the load average by the number of processors on the \
                system."
    )]
    per_cpu: bool,
    #[structopt(
        short = "v",
        long = "verbose",
        help = "print info even if everything is okay"
    )]
    verbose: bool,
}

fn do_check(args: &Args, actual: LoadAvg, num_cpus: usize, per_cpu: bool) -> Status {
    let actual = actual / num_cpus;
    let cpu_str = if per_cpu && num_cpus > 1 {
        format!(" (divided by {} cpus)", num_cpus)
    } else {
        String::new()
    };

    if actual > args.crit {
        println!(
            "[check-load] CRITICAL: load average{} is {} (> {})",
            cpu_str, actual, args.crit
        );
        Status::Critical
    } else if actual > args.warn {
        println!(
            "[check-load] WARNING: load average{} is {} (> {})",
            cpu_str, actual, args.warn
        );
        Status::Warning
    } else {
        if args.verbose {
            println!(
                "[check-load] OK: load average{} is {} (< {})",
                cpu_str, actual, args.warn
            );
        }
        Status::Ok
    }
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args = Args::from_args();

    let num_cpus = if args.per_cpu {
        let cpus = Calculations::load_per_cpu().unwrap();
        cpus.len()
    } else {
        1
    };

    let status = do_check(&args, LoadAvg::load().unwrap(), num_cpus, args.per_cpu);
    status.exit();
}

#[cfg(test)]
mod test {
    use structopt::StructOpt;
    use tabin_plugins::Status;

    use super::{do_check, Args};

    fn build_args(argv: Vec<&str>) -> Args {
        Args::from_iter(argv.into_iter())
    }

    #[test]
    fn flags() {
        let args = build_args(vec!["check-load", "--per-cpu"]);
        assert_eq!(args.per_cpu, true);

        let args = build_args(vec!["check-load"]);
        assert_eq!(args.per_cpu, false);
    }

    #[test]
    fn one_core_statuses() {
        let args = build_args(vec!["check-load", "-c", "1,2,3"]);
        assert_eq!(
            do_check(&args, "2 1 1".parse().unwrap(), 1, args.per_cpu),
            Status::Critical
        );

        let args = build_args(vec!["check-load", "-w", "1,2,3"]);
        assert_eq!(
            do_check(&args, "1.1 1 1".parse().unwrap(), 1, args.per_cpu),
            Status::Warning
        );

        let args = build_args(vec!["check-load"]);
        assert_eq!(
            do_check(&args, "2 1 1".parse().unwrap(), 1, args.per_cpu),
            Status::Ok
        );

        let args = build_args(vec!["check-load"]);
        assert_eq!(
            do_check(&args, "12 1 1".parse().unwrap(), 1, args.per_cpu),
            Status::Critical
        );

        let args = build_args(vec!["check-load", "-c", "2,3,4", "-w", "1,2,3"]);
        assert_eq!(
            do_check(&args, ".5 .5 .5".parse().unwrap(), 1, args.per_cpu),
            Status::Ok
        );
    }

    #[test]
    fn multi_cpu_statuses() {
        let args = build_args(vec!["check-load", "-w", "7,2,2", "-v", "--per-cpu"]);
        assert_eq!(
            do_check(&args, "12 1 1".parse().unwrap(), 2, args.per_cpu),
            Status::Ok
        );

        let args = build_args(vec!["check-load", "-w", "5,2,2", "-v", "--per-cpu"]);
        assert_eq!(
            do_check(&args, "12 1 1".parse().unwrap(), 2, args.per_cpu),
            Status::Warning
        );

        let args = build_args(vec!["check-load", "-v", "--per-cpu"]);
        assert_eq!(
            do_check(&args, "21 1 1".parse().unwrap(), 2, args.per_cpu),
            Status::Critical
        );

        let args = build_args(vec!["check-load", "-v", "--per-cpu", "-c", "6,4,4"]);
        assert_eq!(
            do_check(&args, "13 1 1".parse().unwrap(), 2, args.per_cpu),
            Status::Critical
        );

        let args = build_args(vec!["check-load", "-v", "--per-cpu", "-c", "2,1,1"]);
        assert_eq!(
            do_check(&args, "13 1 1".parse().unwrap(), 4, args.per_cpu),
            Status::Critical
        );
    }
}

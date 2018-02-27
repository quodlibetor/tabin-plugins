//! Check CPU usage of the currently-running container

extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

extern crate tabin_plugins;

use std::thread::sleep;
use std::time::Duration;
use structopt::StructOpt;

use tabin_plugins::Status;
use tabin_plugins::linux::{Jiffies, Ratio};
use tabin_plugins::procfs::{Calculations, LoadProcsError, ProcFsError, RunningProcs};
use tabin_plugins::sys::fs::cgroup::cpuacct::Stat as CGroupStat;
use tabin_plugins::sys::fs::cgroup::cpu::shares;

/// Check the cpu usage of the currently-running container.
///
/// This must be run from inside the container to be checked.
#[derive(Deserialize, StructOpt, Debug)]
#[structopt(name = "check-container-cpu (part of tabin-plugins)",
            raw(setting = "structopt::clap::AppSettings::ColoredHelp"),
            after_help = "About usage percentages:

    If you don't specify '--shares-per-cpu', percentages should be specified
    relative to a single CPU's usage. So if you have a process that you want to
    be allowed to use 4 CPUs worth of processor time, and you were planning on
    going critical at 90%, you should specify something like '--crit 360'

    However, if you are using a container orchestrator such as Mesos, you often
    tell it that you want this container to have '2 CPUs' worth of hardware.
    Your scheduler is responsible for deciding how many cgroup cpu shares 1
    CPU's worth of time is, and keeping track of how many shares it has doled
    out, and then schedule your containers to run with 2 CPUs worth of CPU
    shares. Assuming that your scheduler uses the default number of shares
    (1024) as 'one cpu', this will mean that you have given that cgroup 2048
    shares.

    If you do specify --shares-per-cpu then the percentage that you give will
    be scaled by the number of CPUs worth of shares that this container has
    been given, and CPU usage will be compared to the total percent of the CPUs
    that it has been allocated.

    Which is to say, if you specify --shares-per-cpu, you should always specify
    your warn/crit percentages out of 100%, because this script will correctly
    scale it for your process.

    Here are some examples, where 'shares granted' is the value in
    /sys/fs/cgroup/cpu/cpu.shares:

        * args: --shares-per-cpu 1024 --crit 90
          shares granted: 1024
          percent of one CPU to alert at: 90
        * args: --shares-per-cpu 1024 --crit 90
          shares granted: 2024
          percent of one CPU to alert at: 180
        * args: --shares-per-cpu 1024 --crit 90
          shares granted: 102
          percent of one CPU to alert at: 9
")]
struct Args {
    #[structopt(short = "w", long = "warn", help = "Percent to warn at", default_value = "80")]
    warn: f64,
    #[structopt(short = "c", long = "crit", help = "Percent to go critical at",
                default_value = "80")]
    crit: f64,
    #[structopt(long = "shares-per-cpu",
                help = "The number of CPU shares given to a cgroup when \
                        it has exactly one CPU allocated to it.")]
    shares_per_cpu: Option<u32>,

    #[structopt(short = "s", long = "sample", name = "seconds",
                help = "Seconds to take sample over", default_value = "5")]
    sample: u64,
    #[structopt(long = "show-hogs", name = "count",
                help = "Show <count> most cpu-intensive processes in this container.",
                default_value = "0")]
    show_hogs: usize,
}

fn main() {
    let args: Args = Args::from_args();

    let start_cpu = Calculations::load_per_cpu().unwrap();
    let start_container = CGroupStat::load();
    let mut start_per_proc = None;
    let (cpus, crit, warn) = if let Some(shares_per_cpu) = args.shares_per_cpu {
        let available_cpus = shares().unwrap() as f64 / shares_per_cpu as f64;
        let crit = args.crit * available_cpus;
        let warn = args.warn * available_cpus;
        (Some(available_cpus), crit, warn)
    } else {
        (None, args.crit, args.warn)
    };

    let mut per_proc_errors = vec![];
    if args.show_hogs > 0 {
        start_per_proc = Some(load_procs(&mut per_proc_errors));
    }

    sleep(Duration::from_millis(args.sample * 1000));

    let end_cpu = Calculations::load_per_cpu().unwrap();
    let end_container = CGroupStat::load();

    let median_jiffies = find_median_jiffies_used(&start_cpu, &end_cpu);

    let container_usage = end_container.unwrap().total() - start_container.unwrap().total();
    let percent = container_usage.duration().ratio(&median_jiffies.duration());

    let mut status = Status::Ok;
    let mut cpu_msg = String::new();
    let mut percent_msg = "";
    if let Some(cpus) = cpus {
        cpu_msg = format!(" of {:.1} CPUs", cpus);
        percent_msg = " of 1"
    }
    if percent > crit as f64 {
        println!(
            "CRITICAL: Container is using {}%{} CPU (> {}%{})",
            percent, percent_msg, args.crit, cpu_msg
        );
        status = Status::Critical;
    } else if percent > warn as f64 {
        println!(
            "WARNING: Container is using {}%{} CPU (> {}%{})",
            percent, percent_msg, args.warn, cpu_msg
        );
        status = Status::Warning;
    } else {
        println!(
            "OK: Container is using {}%{} CPU (< {}%{})",
            percent, percent_msg, args.warn, cpu_msg
        );
    }
    if args.show_hogs > 0 {
        let end_per_proc = load_procs(&mut per_proc_errors);
        let start_per_proc = start_per_proc.unwrap();
        let mut per_proc = end_per_proc.percent_cpu_util_since(&start_per_proc, median_jiffies);
        per_proc
            .0
            .sort_by(|l, r| r.total.partial_cmp(&l.total).unwrap());
        println!("INFO [check-container-cpu]: hogs");
        for usage in per_proc.0.iter().take(args.show_hogs) {
            println!(
                "[{:>5}]{:>5.1}%: {}",
                usage.process.stat.pid,
                usage.total,
                usage.process.useful_cmdline()
            );
        }
    }
    status.exit();
}

// These all should be extremely similar to each other, so just taking the
// middle one should be safe
fn find_median_jiffies_used(start: &[Calculations], end: &[Calculations]) -> Jiffies {
    let mut jiffies: Vec<_> = start
        .iter()
        .zip(end)
        .map(|(start, end)| end.total() - start.total())
        .collect();
    jiffies.sort();
    jiffies[jiffies.len() / 2]
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
    use structopt::StructOpt;

    use tabin_plugins::procfs::Calculations; //, RunningProcs};
    use tabin_plugins::linux::Jiffies;
    use super::{find_median_jiffies_used, Args};

    #[test]
    fn opts_parse() {
        let args: Args = Args::from_iter(["arg0", "--crit", "480", "--warn", "20"].into_iter());
        assert_eq!(args.crit, 480.0);
        assert_eq!(args.shares_per_cpu, None);

        let args: Args =
            Args::from_iter(["arg0", "--crit", "480", "--shares-per-cpu", "100"].into_iter());
        assert_eq!(args.crit, 480.0);
        assert_eq!(args.shares_per_cpu, Some(100));
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
    fn median_jiffies_works_with_single_cpu() {
        let start = vec![start()];
        let end = vec![
            Calculations {
                user: Jiffies::new(110),
                ..start[0]
            },
        ];

        let median = find_median_jiffies_used(&start, &end);

        assert_eq!(median, Jiffies::new(10));
    }

    #[test]
    fn median_jiffies_works_with_several_cpus() {
        let mut start_calcs = vec![start(), start(), start(), start()];
        let mut end = vec![
            Calculations {
                user: Jiffies::new(109),
                ..start_calcs[0]
            },
            Calculations {
                user: Jiffies::new(110),
                ..start_calcs[0]
            },
            Calculations {
                user: Jiffies::new(111),
                ..start_calcs[0]
            },
            Calculations {
                user: Jiffies::new(109),
                ..start_calcs[0]
            },
        ];

        let median = find_median_jiffies_used(&start_calcs, &end);

        assert_eq!(median, Jiffies::new(10));

        start_calcs.push(start());
        end.push(Calculations {
            user: Jiffies::new(109),
            ..start_calcs[0]
        });

        let median = find_median_jiffies_used(&start_calcs, &end);
        assert_eq!(median, Jiffies::new(9))
    }
}

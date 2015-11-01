//! Check CPU usage of the currently-running container

extern crate rustc_serialize;
extern crate docopt;

extern crate turbine_plugins;

use docopt::Docopt;

use turbine_plugins::Status;
use turbine_plugins::linux::{Jiffies, Ratio};
use turbine_plugins::procfs::{Calculations, RunningProcs};
use turbine_plugins::sys::fs::cgroup::cpuacct::Stat as CGroupStat;

static USAGE: &'static str = "
Usage:
    check-container-cpu [--one-cpu-share] [options]
    check-cpu (-h | --help)

Options:
    -h, --help            Show this help message

    -w, --warn=<percent>  Percent to warn at           [default: 80]
    -c, --crit=<percent>  Percent to critical at       [default: 80]

    -s, --sample=<secs>   Seconds to take sample over  [default: 5]

    --show-hogs=<count>   Show <count> most cpu-intensive processes in this
                          container.                   [default: 0]

About usage percentages:

    Percentages should be specified relative to a single CPU's usage. So if you
    have a process that you want to be allowed to use 4 CPUs worth of processor
    time, and you were planning on going critical at 90%, you should specify
    something like '--crit 360'
";

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_warn: u32,
    flag_crit: u32,

    flag_sample: u32,
    flag_show_hogs: usize,
}

// These all should be extremely similar to each other, so just taking the
// middle one should be safe
fn find_median_jiffies_used(start: &[Calculations], end: &[Calculations]) -> Jiffies {
    let mut jiffies: Vec<_> = start.iter().zip(end)
        .map(|(start, end)| end.total() - start.total())
        .collect();
    jiffies.sort();
    jiffies[jiffies.len() / 2]
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    let start_cpu = Calculations::load_per_cpu().unwrap();
    let start_container = CGroupStat::load();
    let mut start_per_proc = None;
    if args.flag_show_hogs > 0 {
        start_per_proc = Some(RunningProcs::currently_running().unwrap());
    }

    std::thread::sleep_ms(args.flag_sample * 1000);

    let end_cpu = Calculations::load_per_cpu().unwrap();
    let end_container = CGroupStat::load();

    let median_jiffies = find_median_jiffies_used(&start_cpu, &end_cpu);

    let container_usage = end_container.unwrap().total() - start_container.unwrap().total();
    let percent = container_usage.duration()
        .ratio(&median_jiffies.duration());

    let mut status = Status::Ok;
    if percent > args.flag_crit as f64 {
        println!("CRITICAL: Container is using {}% CPU (> {}%)", percent, args.flag_crit);
        status = Status::Critical;
    } else if percent > args.flag_warn as f64 {
        println!("WARNING: Container is using {}% CPU (> {}%)", percent, args.flag_warn);
        status = Status::Warning;
    } else {
        println!("OK: Container is using {}% CPU (< {}%)", percent, args.flag_warn);
    }
    if args.flag_show_hogs > 0 {
        let end_per_proc = RunningProcs::currently_running().unwrap();
        let start_per_proc = start_per_proc.unwrap();
        let mut per_proc = end_per_proc.percent_cpu_util_since(
            &start_per_proc,
            median_jiffies);
        per_proc.0.sort_by(|l, r| r.total.partial_cmp(&l.total).unwrap());
        println!("INFO [check-container-cpu]: hogs");
        for usage in per_proc.0.iter().take(args.flag_show_hogs) {
            println!("[{:>5}]{:>5.1}%: {}",
                     usage.process.stat.pid,
                     usage.total,
                     usage.process.useful_cmdline());
        }
    }
    status.exit();
}

#[cfg(test)]
mod unit {
    use docopt::Docopt;
    use turbine_plugins::procfs::{Calculations}; //, RunningProcs};
    use turbine_plugins::linux::Jiffies;
    use super::{USAGE, Args, find_median_jiffies_used};

    #[test]
    fn opts_parse() {
        let args: Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(vec!["arg0", "--crit", "480", "--warn", "20"].into_iter())
                      .decode())
            .unwrap();
        assert_eq!(args.flag_crit, 480);
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
        let start = vec![
            start(),
            ];
        let end = vec![
            Calculations {
                user: Jiffies::new(110),
                ..start[0]
            }];

        let median = find_median_jiffies_used(&start, &end);

        assert_eq!(median, Jiffies::new(10));
    }

    #[test]
    fn median_jiffies_works_with_several_cpus() {
        let mut start_calcs = vec![
            start(),
            start(),
            start(),
            start(),
            ];
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

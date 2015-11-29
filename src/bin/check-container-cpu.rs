//! Check CPU usage of the currently-running container

extern crate rustc_serialize;
extern crate docopt;

extern crate turbine_plugins;

use std::thread::sleep;
use std::time::Duration;

use docopt::Docopt;

use turbine_plugins::Status;
use turbine_plugins::linux::{Jiffies, Ratio};
use turbine_plugins::procfs::{Calculations, RunningProcs};
use turbine_plugins::sys::fs::cgroup::cpuacct::Stat as CGroupStat;
use turbine_plugins::sys::fs::cgroup::cpu::shares;

static USAGE: &'static str = r#"
Usage:
    check-container-cpu [options]
    check-cpu (-h | --help)

Check the cpu usage of the currently-running container. This must be run from
inside the container to be checked.

Options:
    -h, --help            Show this help message

    -w, --warn=<percent>  Percent to warn at           [default: 80]
    -c, --crit=<percent>  Percent to critical at       [default: 80]

    -s, --sample=<secs>   Seconds to take sample over  [default: 5]

    --show-hogs=<count>   Show <count> most cpu-intensive processes in this
                          container.                   [default: 0]

    --shares-per-cpu=<shares>
                          The number of CPU shares given to a cgroup when
                          it has exactly one CPU allocated to it.

About usage percentages:

    If you don't specify '--shares-per-cpu', percentages should be specified
    relative to a single CPU's usage. So if you have a process that you want to
    be allowed to use 4 CPUs worth of processor time, and you were planning on
    going critical at 90%, you should specify something like '--crit 360'

    However, if you are using a container orchestrator such as Mesos, you often
    tell it that you want this container to have "2 CPUs" worth of hardware.
    Your scheduler is responsible for deciding how many cgroup cpu shares 1
    CPU's worth of time is, and keeping track of how many shares it has doled
    out, and then schedule your containers to run with 2 CPUs worth of CPU
    shares. Assuming that your scheduler uses the default number of shares
    (1024) as "one cpu", this will mean that you have given that cgroup 2048
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

"#;

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_warn: u32,
    flag_crit: u32,
    flag_shares_per_cpu: Option<u32>,

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
    let (mut crit, mut warn) = (args.flag_crit, args.flag_warn);
    let mut cpus = None;
    if let Some(shares_per_cpu) = args.flag_shares_per_cpu {
        let available_cpus = shares().unwrap() as f64 / shares_per_cpu as f64;
        crit = (crit as f64 * available_cpus) as u32;
        warn = (warn as f64 * available_cpus) as u32;
        cpus = Some(available_cpus)
    }

    if args.flag_show_hogs > 0 {
        start_per_proc = Some(RunningProcs::currently_running().unwrap());
    }

    sleep(Duration::from_millis(args.flag_sample as u64 * 1000));

    let end_cpu = Calculations::load_per_cpu().unwrap();
    let end_container = CGroupStat::load();

    let median_jiffies = find_median_jiffies_used(&start_cpu, &end_cpu);

    let container_usage = end_container.unwrap().total() - start_container.unwrap().total();
    let percent = container_usage.duration()
        .ratio(&median_jiffies.duration());

    let mut status = Status::Ok;
    let mut cpu_msg = String::new();
    let mut percent_msg = "";
    if let Some(cpus) = cpus {
        cpu_msg = format!(" of {:.1} CPUs", cpus);
        percent_msg = " of 1"
    }
    if percent > crit as f64 {
        println!("CRITICAL: Container is using {}%{} CPU (> {}%{})",
                 percent, percent_msg, args.flag_crit, cpu_msg);
        status = Status::Critical;
    } else if percent > warn as f64 {
        println!("WARNING: Container is using {}%{} CPU (> {}%{})",
                 percent, percent_msg, args.flag_warn, cpu_msg);
        status = Status::Warning;
    } else {
        println!("OK: Container is using {}%{} CPU (< {}%{})",
                 percent, percent_msg, args.flag_warn, cpu_msg);
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
        assert_eq!(args.flag_shares_per_cpu, None);

        let args: Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(vec!["arg0", "--crit", "480", "--shares-per-cpu", "100"].into_iter())
                      .decode())
            .unwrap();
        assert_eq!(args.flag_crit, 480);
        assert_eq!(args.flag_shares_per_cpu, Some(100));
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

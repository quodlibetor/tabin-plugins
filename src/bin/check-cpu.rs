//! Check CPU usage

extern crate rustc_serialize;

extern crate docopt;

extern crate turbine_plugins;

use docopt::Docopt;
use turbine_plugins::ExitStatus;

use std::fs::File;
use std::io::{BufReader,Read};
use std::path::Path;

static USAGE: &'static str = "
Usage: check-cpu [options] [--type=<work-source>...]

Options:
    -h, --help               Show this help message

    -s, --sample=<seconds>   Seconds to spent collecting   [default: 1]
    -w, --warn=<percent>     Percent to warn at            [default: 80]
    -c, --crit=<percent>     Percent to critical at        [default: 95]

CPU Work Types:

    Specifying one of the CPU kinds checks that kind of utilization. The
    default is to check total utilization.

    --type=<usage>           Some of:
                                total user nice system idle
                                iowait irq softirq steal guest [default: total]
";

#[derive(RustcDecodable, Debug)]
enum WorkSource {
    Total, User, Nice, System, Idle, IoWait, Irq, SoftIrq, Steal, Guest, GuestNice
}

impl std::fmt::Display for WorkSource {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use WorkSource::*;
        let s = match *self {
            Total => "total",
            User => "user",
            Nice => "nice",
            System => "system",
            Idle => "idle",
            IoWait => "iowait",
            Irq => "irq",
            SoftIrq => "softirq",
            Steal => "steal",
            Guest => "guest",
            GuestNice => "guestnice"
        };
        f.write_str(s)
    }
}

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_help: bool,
    flag_sample: i32,
    flag_warn: isize,
    flag_crit: isize,

    flag_type: Vec<WorkSource>,
}

/// The number of calculations that have occured on this computer since it
/// started
#[derive(Debug)]
struct Calculations {
    user: f64,
    nice: f64,
    system: f64,
    idle: f64,
    iowait: f64,
    irq: f64,
    softirq: f64,
    steal: f64,
    guest: f64,
    guest_nice: Option<f64>,
}

impl Calculations {
    /// Jiffies spent non-idle
    ///
    /// This includes all processes in user space, kernel space, and time
    /// stolen by other VMs.
    fn active(&self) -> f64 {
        self.user + self.nice // user processes
            + self.system + self.irq + self.softirq // kernel and interrupts
            + self.steal  // other VMs stealing our precious time
    }

    /// Jiffies spent with nothing to do
    fn idle(&self) -> f64 {
        self.idle + self.iowait
    }

    /// Jiffies spent running child VMs
    ///
    /// This is included in `active()`, so don't add this to that when
    /// totalling.
    #[allow(dead_code)]  // this mostly exists as documentation of what `guest` means
    fn virt(&self) -> f64 {
        self.guest + self.guest_nice.unwrap_or(0.0)
    }

    /// All jiffies since the kernel started tracking
    fn total(&self) -> f64 {
        self.active() + self.idle()
    }
}

/// Return how much cpu time the specific worksource took
///
/// The number returned is between 0 and 100
fn percent_util(kind: &WorkSource, start: &Calculations, end: &Calculations) -> f64 {
    let (start_val, end_val) = match *kind {
        WorkSource::Total => (start.active(), end.active()),
        WorkSource::User => (start.user, end.user),
        WorkSource::IoWait => (start.iowait, end.iowait),
        WorkSource::Nice => (start.nice, end.nice),
        WorkSource::System => (start.system, end.system),
        WorkSource::Idle => (start.idle, end.idle),
        WorkSource::Irq => (start.irq, end.irq),
        WorkSource::SoftIrq => (start.softirq, end.softirq),
        WorkSource::Steal => (start.steal, end.steal),
        WorkSource::Guest => (start.guest, end.guest),
        WorkSource::GuestNice => (start.guest_nice.unwrap_or(0f64), end.guest_nice.unwrap_or(0f64)),
    };
    let total = (end_val - start_val) /
        (end.total() - start.total());
    total * 100f64
}

fn read_cpu() -> Calculations {
    let contents = match File::open(&Path::new("/proc/stat")) {
        Ok(ref mut content) => {
            let mut s = String::new();
            let _ = BufReader::new(content).read_to_string(&mut s);
            s
        },
        Err(e) => panic!("Unable to read /proc/stat: {:?}", e)
    };
    let mut word = String::new();
    let mut usages = Vec::new();
    for chr in contents.chars() {
        match chr {
            ' ' => {
                if word != "" && word != "cpu" {
                    let usage = match word.parse() {
                        Ok(num) => num,
                        Err(e) => panic!("Unable to parse '{}' as f64: {:?}", word, e)
                    };
                    usages.push(usage)
                };
                word.clear();
            },
            '\n' => break,
            _ => word.push(chr)
        }
    }

    Calculations {
        user: usages[0],
        nice: usages[1],
        system: usages[2],
        idle: usages[3],
        iowait: usages[4],
        irq: usages[5],
        softirq: usages[6],
        steal: usages[7],
        guest: usages[8],
        guest_nice: match usages.get(9) {
            Some(n) => Some(n.clone()),
            None => None
        },
    }
}

fn do_comparison(args: &Args, start: &Calculations, end: &Calculations) -> ExitStatus {
    let mut exit_status = ExitStatus::Ok;

    for flag in &args.flag_type {
        let total = percent_util(flag, &start, &end);
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

    let start = read_cpu();
    std::thread::sleep_ms((args.flag_sample * 1000) as u32);
    let end = read_cpu();
    let status = do_comparison(&args, &start, &end);
    status.exit()
}

#[cfg(test)]
mod unit {
    use docopt::Docopt;
    use super::{USAGE, WorkSource, Calculations, percent_util};

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

        assert_eq!(percent_util(&WorkSource::User, &start, &end), 50.0)
    }

    #[test]
    fn percentage_user() {
        let start = begintime();

        let end = Calculations {
            user: 110.0,
            ..start
        };

        assert_eq!(percent_util(&WorkSource::User, &start, &end), 100.0)
    }

    #[test]
    fn percentage_total_user_idle() {
        let start = begintime();
        let end = Calculations {
            user: 110.0,
            idle: 110.0,
            ..start
        };

        assert_eq!(percent_util(&WorkSource::Total, &start, &end), 50.0)
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

        assert_eq!(percent_util(&WorkSource::Total, &start, &end), 75.0)
    }
}

#[cfg(test)]
mod integration {
    // not really integration tests, but higher level
    use super::{do_comparison, Calculations, USAGE};

    use turbine_plugins::ExitStatus;
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

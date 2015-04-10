//! This is a documentation.

#![feature(exit_status)]

extern crate rustc_serialize;

extern crate docopt;

use docopt::Docopt;

use std::fs::File;
use std::io::{BufReader,Read};
use std::path::Path;

static USAGE: &'static str = "
Usage: check-cpu [options] [--type=<work-source>]

Options:
    -h, --help             Show this help message

    -s, --sleep=<seconds>  Seconds to collect for [default: 1]
    -w, --warn=<percent>   Percent to warn at     [default: 80]
    -c, --crit=<percent>   Percent to critical at [default: 95]

CPU Work Types:
    Specifying one of the CPU kinds checks that kind of utilization. The
    default is to check total utilization.

    --type=<usage>  One of:
                       total user nice system idle
                       iowait irq softirq steal guest [default: total]
";

#[derive(RustcDecodable, Debug)]
enum WorkSource {
    Total, User, Nice, System, Idle, IoWait, Irq, SoftIrq, Steal, Guest, GuestNice
}

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_help: bool,
    flag_sleep: i32,
    flag_warn:  isize,
    flag_crit:  isize,

    flag_type: WorkSource,
}

/// The number of calculations that have occured on this computer since it
/// started
#[derive(Debug)]
pub struct Calculations {
    pub user: f64,
    pub nice: f64,
    pub system: f64,
    pub idle: f64,
    pub iowait: f64,
    pub irq: f64,
    pub softirq: f64,
    pub steal: f64,
    pub guest: f64,
    pub guest_nice: Option<f64>,
}

impl Calculations {
    pub fn sub(&self, other: &Calculations) -> Calculations {
        Calculations {
            user: self.user - other.user,
            nice: self.nice - other.nice,
            system: self.system - other.system,
            idle: self.idle - other.idle,
            iowait: self.iowait - other.iowait,
            irq: self.irq - other.irq,
            softirq: self.softirq - other.softirq,
            steal: self.steal - other.steal,
            guest: self.guest - other.guest,
            guest_nice: match (self.guest_nice, other.guest_nice) {
                (Some(snice), Some(onice)) => Some(snice - onice),
                _ => None
            }
        }
    }

    pub fn total(&self) -> f64 {
        self.user + self.nice + self.system + self.idle + self.iowait +
            self.irq + self.softirq + self.steal + self.guest +
            self.guest_nice.unwrap_or(0f64)
    }
}

fn percent_util(kind: &WorkSource, start: &Calculations, end: &Calculations) -> f64 {
    let (start_val, end_val) = match *kind {
        WorkSource::Total => ((start.user + start.nice + start.system + start.iowait + start.irq + start.steal),
                              (end.user + end.nice + end.system + end.iowait + end.irq + end.steal)),
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

pub fn read_cpu() -> Calculations {
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

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());
    if args.flag_help {
        print!("{}", USAGE);
        return;
    }

    let start = read_cpu();
    std::thread::sleep_ms((args.flag_sleep * 1000) as u32);
    let end = read_cpu();
    let total = percent_util(&args.flag_type, &start, &end);
    if total > args.flag_crit as f64 {
        std::env::set_exit_status(2);
        println!("check-cpu critical: {:?} > {}", total, args.flag_crit);
    } else if total > args.flag_warn as f64 {
        std::env::set_exit_status(1);
        println!("check-cpu warning: {} > {}", total, args.flag_warn);
    } else {
        println!("check-cpu ok");
    }
    println!("{:?}={:?} ({:?})", args.flag_type, total, end.sub(&start))
}

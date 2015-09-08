//! Check the load average of the system
//!
//! Load average is the number of processes *waiting* to do work in a queue, either
//! due to IO or CPU constraints. The numbers used to check are the load averaged
//! over 1, 5 and 15 minutes, respectively

extern crate rustc_serialize;

extern crate docopt;

extern crate turbine_plugins;

use docopt::Docopt;
use turbine_plugins::Status;
use turbine_plugins::procfs::{Calculations, LoadAvg};

static USAGE: &'static str = "
Usage: check-load [options]
       check-load -h | --help

Check the load average of the system

Load average is the number of processes *waiting* to do work in a queue, either
due to IO or CPU constraints. The numbers used to check are the load averaged
over 1, 5 and 15 minutes, respectively

Options:
    -h, --help              Show this message and exit
    -v, --verbose           Print even when things are okay

Threshold Behavior:
    -w, --warn=<averages>   Averages to warn at         [default: 5,3.5,2.5]
    -c, --crit=<averages>   Averages to go critical at  [default: 10,5,3]

    --per-cpu=<maybe>      Divide the load average by the number of processors on the
                           system.                      [default: true]
";


#[derive(RustcDecodable, Debug)]
struct RawArgs {
    flag_warn: String,
    flag_crit: String,
    flag_per_cpu: bool,
    flag_verbose: bool,
}

struct Args {
    flag_warn: LoadAvg,
    flag_crit: LoadAvg,
    flag_per_cpu: bool,
    flag_verbose: bool,
}

impl From<RawArgs> for Args {
    fn from(args: RawArgs) -> Args {
        Args {
            flag_warn: args.flag_warn.parse().ok().expect(
                &format!("--warn should look like 'n.m,n.m,n.m' not {}", args.flag_warn)),
            flag_crit: args.flag_crit.parse().ok().expect(
                &format!("--crit should look like 'n.m,n.m,n.m' not {}", args.flag_warn)),
            flag_per_cpu: args.flag_per_cpu,
            flag_verbose: args.flag_verbose,
        }
    }
}

fn parse_args() -> Result<Args, docopt::Error> {
    let args: RawArgs = try!(Docopt::new(USAGE)
                             .and_then(|d| d.help(true).decode()));
    Ok(args.into())
}

fn do_check(args: Args, actual: LoadAvg, num_cpus: usize) -> Status {
    let actual = actual / num_cpus;

    if actual > args.flag_crit {
        println!("[check-load] CRITICAL: load average is {} (> {})",
                 actual, args.flag_crit);
        Status::Critical
    } else if actual > args.flag_warn {
        println!("[check-load] WARNING: load average is {} (> {})",
                 actual, args.flag_warn);
        Status::Warning
    } else {
        if args.flag_verbose {
            println!("[check-load] OK: load average is {} (< {})",
                 actual, args.flag_warn);
        }
        Status::Ok
    }
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args = parse_args().unwrap_or_else(|e| e.exit());

    let num_cpus = if args.flag_per_cpu {
        let cpus = Calculations::load_per_cpu().unwrap();
        cpus.len()
    } else {
        1
    };

    let status = do_check(args, LoadAvg::load().unwrap(), num_cpus);
    status.exit();
}


#[cfg(test)]
mod test {
    use docopt::Docopt;
    use turbine_plugins::Status;

    use super::{USAGE, RawArgs, Args, parse_args, do_check};

    #[test] // one day syntax extensions won't require nightly.
    fn parse_args_is_valid() {
        parse_args().unwrap();
    }

    fn docopt(argv: Vec<&str>) -> Args {
        Args::from(Docopt::new(USAGE)
                   .and_then(|d| d.argv(argv.into_iter()).decode::<RawArgs>())
                   .unwrap())
    }

    #[test]
    fn statuses() {
        let args = docopt(vec!["check-load", "-c", "1,2,3"]);
        assert_eq!(
            do_check(args,
                     "2 1 1".parse().unwrap(),
                     1),
            Status::Critical
        );

        let args = docopt(vec!["check-load", "-w", "1,2,3"]);
        assert_eq!(
            do_check(args,
                     "1.1 1 1".parse().unwrap(),
                     1),
            Status::Warning
        );

        let args = docopt(vec!["check-load"]);
        assert_eq!(
            do_check(args,
                     "2 1 1".parse().unwrap(),
                     1),
            Status::Ok
        );

        let args = docopt(vec!["check-load"]);
        assert_eq!(
            do_check(args,
                     "12 1 1".parse().unwrap(),
                     1),
            Status::Critical
        );

        let args = docopt(vec!["check-load", "-w", "7,2,2"]);
        assert_eq!(
            do_check(args,
                     "12 1 1".parse().unwrap(),
                     2),
            Status::Ok
        );


        let args = docopt(vec!["check-load", "-c", "2,3,4", "-w", "1,2,3"]);
        assert_eq!(
            do_check(args,
                     ".5 .5 .5".parse().unwrap(),
                     1),
            Status::Ok
        );
    }
}
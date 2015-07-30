//! Check the load average of the system
//!
//! Load average is the number of processes *waiting* to do work in a queue,
//! either due to IO or CPU constraints, averaged over 1, 5 and 15 minutes.

extern crate rustc_serialize;

extern crate docopt;

extern crate turbine_plugins;

use docopt::Docopt;
use turbine_plugins::ExitStatus;
use turbine_plugins::procfs::{LoadAvg};

static USAGE: &'static str = "
Usage: check-load [options]

Options:
    -h, --help              Show this message and exit
    -v, --verbose           Print even when things are okay

Threshold Behavior:
    -w, --warn=<averages>   Averages to warn at         [default: 5,3.5,2.5]
    -c, --crit=<averages>   Averages to go critical at  [default: 10,5,3]
";


#[derive(RustcDecodable, Debug)]
struct RawArgs {
    flag_warn: String,
    flag_crit: String,
    flag_verbose: bool,
}

struct Args {
    flag_warn: LoadAvg,
    flag_crit: LoadAvg,
    flag_verbose: bool,
}

impl From<RawArgs> for Args {
    fn from(args: RawArgs) -> Args {
        Args {
            flag_warn: args.flag_warn.parse().ok().expect(
                &format!("--warn should look like 'n.m,n.m,n.m' not {}", args.flag_warn)),
            flag_crit: args.flag_crit.parse().ok().expect(
                &format!("--crit should look like 'n.m,n.m,n.m' not {}", args.flag_warn)),
            flag_verbose: args.flag_verbose,
        }
    }
}

fn parse_args() -> Result<Args, docopt::Error> {
    let args: RawArgs = try!(Docopt::new(USAGE)
                             .and_then(|d| d.help(true).decode()));
    Ok(args.into())
}

fn do_check(args: Args, actual: LoadAvg) -> ExitStatus {
    if actual > args.flag_crit {
        println!("[check-load] CRITICAL: load average is {} (> {})",
                 actual, args.flag_crit);
        ExitStatus::Critical
    } else if actual > args.flag_warn {
        println!("[check-load] WARNING: load average is {} (> {})",
                 actual, args.flag_warn);
        ExitStatus::Warning
    } else {
        if args.flag_verbose {
            println!("[check-load] OK: load average is {} (< {})",
                 actual, args.flag_warn);
        }
        ExitStatus::Ok
    }
}

fn main() {
    let args = parse_args().unwrap_or_else(|e| e.exit());;
    let status = do_check(args, LoadAvg::load().unwrap());
    status.exit();
}


#[cfg(test)]
mod test {
    use docopt::Docopt;
    use turbine_plugins::ExitStatus;

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
                     "2 1 1".parse().unwrap()),
            ExitStatus::Critical
        );

        let args = docopt(vec!["check-load", "-w", "1,2,3"]);
        assert_eq!(
            do_check(args,
                     "1.1 1 1".parse().unwrap()),
            ExitStatus::Warning
        );

        let args = docopt(vec!["check-load"]);
        assert_eq!(
            do_check(args,
                     "2 1 1".parse().unwrap()),
            ExitStatus::Ok
        );

        let args = docopt(vec!["check-load"]);
        assert_eq!(
            do_check(args,
                     "12 1 1".parse().unwrap()),
            ExitStatus::Critical
        );

        let args = docopt(vec!["check-load", "-c", "2,3,4", "-w", "1,2,3"]);
        assert_eq!(
            do_check(args,
                     ".5 .5 .5".parse().unwrap()),
            ExitStatus::Ok
        );
    }
}

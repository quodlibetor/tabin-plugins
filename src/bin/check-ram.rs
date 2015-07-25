extern crate rustc_serialize;

extern crate docopt;
extern crate turbine_plugins;

use docopt::Docopt;

use turbine_plugins::procfs::MemInfo;

static USAGE: &'static str = "
Usage: check-ram [options]

Options:
    -h, --help             Show this help message

    -w, --warn=<percent>   Percent to warn at     [default: 80]
    -c, --crit=<percent>   Percent to critical at [default: 95]
";

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_help: bool,
    flag_warn:  f64,
    flag_crit:  f64,
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
    let mem = MemInfo::load();
    println!("Got some meminfo: {:?}", mem);
    match mem.percent_used() {
        Ok(percent) => if percent > args.flag_crit {
            println!("check-ram critical: {:.1}% > {}%", percent, args.flag_crit);
            std::process::exit(2);
        } else if percent > args.flag_warn {
            println!("check-ram warning: {:.1}% > {}%", percent, args.flag_warn)
        } else {
            println!("check-ram okay: {:.1}% < {}%", percent, args.flag_warn)
        },
        Err(e) => {
            println!("check-ram UNEXPECTED ERROR: {:?}", e);
            std::process::exit(3)
        }
    }
}

#[cfg(test)]
mod test {
    use docopt::Docopt;
    use super::{USAGE, Args};

    #[test]
    fn usage_is_valid() {
        let _: Args = Docopt::new(USAGE).and_then(|d| d.decode()).unwrap();
    }
}

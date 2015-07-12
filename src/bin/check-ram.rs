extern crate rustc_serialize;

extern crate docopt;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use docopt::Docopt;

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

#[derive(PartialEq, Eq, Debug)]
pub struct MemInfo {
    total: Option<usize>,
    available: Option<usize>,
    free: Option<usize>,
    cached: Option<usize>
}

#[derive(Debug)]
enum RamError {
    InsufficientData(String)
}

impl MemInfo {
    /// Try to figure out how much memory is being used
    ///
    /// Since linux kernel 3.16 this just performs
    ///
    ///     (total - available) / total * 100
    ///
    /// Before that we approximate available as Free + Cached, even though that
    /// is [almost certain to be incorrect][].
    ///
    /// [almost certain to be incorrect]: https://github.com/torvalds/linux/commit/34e431b0ae398fc54ea69ff85ec700722c9da773
    fn percent_free(&self) -> Result<f64, RamError> {
        match *self {
            MemInfo { total: Some(t), available: Some(a), .. } => {
                Ok( (t - a) as f64 / t as f64 * 100f64 )
            }
            MemInfo { total: Some(t), free: Some(f), cached: Some(c), ..} => {
                Ok( (t - (f + c)) as f64 / t as f64) },
            _ => { Err(RamError::InsufficientData(format!("/proc/meminfo is missing one of total, available, free, or cached: {:?}", self)))}
        }
    }

    fn percent_used(&self) -> Result<f64, RamError> {
        let free = try!(self.percent_free());
        Ok(100f64 - free)
    }

    /// Convert the contents of a string like /proc/meminfo into a MemInfo
    /// object
    fn from_str(meminfo: &str) -> Self {
        let mut word = String::new();
        let mut info = MemInfo {
            total: None,
            free: None,
            available: None,
            cached: None
        };
        let mut amount: usize;
        enum Currently {
            Total, Free, Available, Cached, Unknown, None
        };
        let mut currently = Currently::None;

        for chr in meminfo.chars() {
            // println!("'a'<= {}: {}, {} <= 'Z': {}",
            //        chr, 'A' <= chr, chr, chr <= 'z');
            match chr {
                c if 'A' <= c && c <= 'z' => {
                    word.push(chr);
                },
                ':' => {
                    match &word[..] {
                        "MemTotal" => currently = Currently::Total,
                        "MemAvailable" => currently = Currently::Available,
                        "MemFree" => currently = Currently::Free,
                        "Cached" => currently = Currently::Cached,
                        _ => currently = Currently::Unknown
                    };
                    word.clear();
                }
                x if '0' <= x && x <= '9' => {
                    word.push(chr);
                },
                ' ' | '\n' => {
                    if word.is_empty() { continue };
                    if word == "kB" { word.clear(); continue; };

                    amount = match word.parse() {
                        Ok(amount) => amount,
                        Err(e) => panic!(r#"Unable to parse number from "{}": {:?}"#, word, e)
                    };
                    word.clear();

                    match currently {
                        Currently::Total => info.total = Some(amount),
                        Currently::Free => info.free = Some(amount),
                        Currently::Available => info.available = Some(amount),
                        Currently::Cached => info.cached = Some(amount),
                        Currently::Unknown => { /* don't care */ },
                        Currently::None => {
                            panic!(
                                "Unexpectedly parsed a number before figuring out where I am: {}",
                                amount)
                        }
                    }
                }
                _ => { /* Don't care about other chars */ }
            }
        }

        info
    }
}

fn read_mem() -> MemInfo {
    let contents = match File::open(&Path::new("/proc/meminfo")) {
        Ok(ref mut content) => {
            let mut s = String::new();
            let _ = content.read_to_string(&mut s);
            s
        }
        Err(e) => panic!("Unable to /proc/meminfo: {:?}", e)
    };

    MemInfo::from_str(&contents)
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
    let mem = read_mem();
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
    use super::MemInfo;

    #[test]
    fn parse_meminfo() {
        assert_eq!(
            MemInfo::from_str(
"Useless: 898
MemTotal: 500
MemAvailable: 20
MemFree: 280
Meaningless: 777
Cached: 200
"),
            MemInfo {
                total: Some(500),
                available: Some(20),
                free: Some(280),
                cached: Some(200)
            }
        )
    }
}

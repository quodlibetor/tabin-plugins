//! Check Disk usage

#[macro_use]
extern crate derive_more;
extern crate env_logger;
#[macro_use]
extern crate log;
extern crate nix;
extern crate regex;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

extern crate tabin_plugins;

use std::cmp::max;
use std::collections::HashSet;
use std::fmt;

use nix::sys::statvfs::vfs;
use regex::Regex;
use structopt::StructOpt;
use tabin_plugins::linux::bytes_to_human_size;
use tabin_plugins::procfs::Mount;
use tabin_plugins::Status;

/// Check all mounted file systems for disk usage.
///
/// For some reason this check generally generates values that are between 1% and
/// 3% higher than `df`, even though AFAICT we're both just calling statvfs a bunch
/// of times.
#[derive(StructOpt, Deserialize, Debug)]
#[structopt(
    name = "check-disk (part of tabin-plugins)",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct Args {
    #[structopt(
        short = "w",
        long = "warn",
        help = "Percent to warn at",
        default_value = "80"
    )]
    warn: f64,
    #[structopt(
        short = "c",
        long = "crit",
        help = "Percent to go critical at",
        default_value = "90"
    )]
    crit: f64,
    #[structopt(
        short = "W",
        long = "warn-inodes",
        help = "Percent of inode usage to warn at",
        default_value = "80"
    )]
    warn_inodes: f64,
    #[structopt(
        short = "C",
        long = "crit-inodes",
        help = "Percent of inode usage to go critical at",
        default_value = "90"
    )]
    crit_inodes: f64,

    #[structopt(
        long = "pattern",
        name = "regex",
        help = "Only check filesystems that match this regex"
    )]
    pattern: Option<String>,
    #[structopt(
        long = "exclude-pattern",
        name = "exclude-regex",
        help = "Only check filesystems that match this regex"
    )]
    exclude_pattern: Option<String>,
    #[structopt(
        long = "type",
        name = "fs-type",
        help = "Only check filesystems that are of this type, e.g. \
                ext4 or tmpfs. See 'man 8 mount' for more examples."
    )]
    fs_type: Option<String>,
    #[structopt(
        long = "exclude-type",
        name = "exclude-fs-type",
        help = "Do not check filesystems that are of this type."
    )]
    exclude_type: Option<String>,
    #[structopt(
        long = "info",
        help = "Print information of all known filesystems. \
                Similar to df."
    )]
    info: bool,
    // df defaults to ignoring innaccessible filesystems, so we should too
    #[structopt(
        long = "inaccessible-status",
        name = "STATUS",
        help = "If any filesystems are inaccessible print a warning and exit with STATUS. \
                Choices: [critical, warning, ok]"
    )]
    inaccessible_status: Option<Status>,
}

const LOG_VAR: &str = "TABIN_LOG";

fn main() {
    let args = Args::from_args();
    env_logger::Builder::from_env(LOG_VAR).init();

    let mut mounts = match Mount::load_all() {
        Ok(mounts) => mounts,
        Err(e) => {
            println!("CRITICAL error loading mounts: {}", e);
            Status::Critical.exit();
        }
    };
    mounts.sort_by(|l, r| l.file.len().cmp(&r.file.len()));

    let status = match filter(mounts, &args) {
        Ok(ms) => do_check(&ms, &args),
        Err(Error::NotAccessible {
            accessible,
            not_accessible,
        }) => {
            let check_result = do_check(&accessible, &args);
            if args.inaccessible_status.is_some() {
                let status = args.inaccessible_status.unwrap();
                println!(
                    "{}: {} filesystems were not accessible, \
                     run with TABIN_LOG=debug for details",
                    status, not_accessible
                );
                max(check_result, status)
            } else {
                check_result
            }
        }
        Err(e) => {
            println!("{}", e);
            Status::Critical
        }
    };

    status.exit();
}

#[derive(Debug, PartialEq, Eq)]
struct ErrorMsg {
    msg: String,
}

#[derive(Debug, From)]
enum Error {
    Message(ErrorMsg),
    NotAccessible {
        accessible: Vec<MountStat>,
        not_accessible: u32,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            Error::Message(ref e) => write!(f, "{}", e.msg),
            Error::NotAccessible {
                ref not_accessible, ..
            } => write!(f, "{} directories were not accesible", not_accessible),
        }
    }
}

type DiskResult<T> = Result<T, Error>;

#[derive(Debug)]
struct MountStat {
    mount: Mount,
    stat: vfs::Statvfs,
}

fn percent(part: u64, whole: u64) -> f64 {
    100.0 - (part as f64 / whole as f64) * 100.0
}

fn maybe_regex(pattern: &Option<String>) -> DiskResult<Option<Regex>> {
    if let Some(ref pattern) = *pattern {
        let re = match Regex::new(&pattern) {
            Ok(re) => re,
            Err(e) => {
                return Err(ErrorMsg {
                    msg: format!("Unable to filter disks like {:?}: {}", pattern, e),
                }
                .into());
            }
        };
        Ok(Some(re))
    } else {
        Ok(None)
    }
}

/// Convert Mounts into MountStats, applying filters from args
///
/// This:
///
/// * calls `statvfs` on every filesystem in `/proc/mounts`
/// * filters out dummy (0-block) and /proc filesystems
/// * Only shows one of any given `/dev/` filesystem's mount points (the
///   shortest, same as df)
/// * Applies the `pattern` and `type` filters
fn filter(mounts: Vec<Mount>, args: &Args) -> DiskResult<Vec<MountStat>> {
    let mut devices = HashSet::new();
    let include_regex = maybe_regex(&args.pattern)?;
    let exclude_regex = maybe_regex(&args.exclude_pattern)?;
    let mut error_count = 0;
    let ms = mounts
        .into_iter()
        .filter(|mount| {
            include_regex
                .as_ref()
                .map(|re| re.is_match(&mount.file))
                .unwrap_or(true)
        })
        .filter(|mount| {
            exclude_regex
                .as_ref()
                .map(|re| !re.is_match(&mount.file))
                .unwrap_or(true)
        })
        .filter_map(|mount| {
            let stat = match vfs::Statvfs::for_path(mount.file.as_bytes()) {
                Ok(stat) => stat,
                Err(e) => {
                    error_count += 1;
                    debug!("Error reading statvfs for path {}: {}", mount.file, e);
                    return None;
                }
            };
            if stat.f_blocks > 0 && !mount.file.starts_with("/proc") {
                Some(MountStat {
                    mount: mount,
                    stat: stat,
                })
            } else {
                None
            }
        })
        // If the same device is mounted multiple times, we only want one. In
        // To match df we expect these to come in in sorted order and just keep
        // the first one.
        .filter_map(|ms| {
            if ms.mount.spec.starts_with("/dev") {
                if devices.contains(&ms.mount.spec) {
                    None
                } else {
                    devices.insert(ms.mount.spec.clone());
                    Some(ms)
                }
            } else {
                Some(ms)
            }
        })
        .filter(|ms| {
            args.fs_type
                .as_ref()
                .map(|vfstype| ms.mount.vfstype == *vfstype)
                .unwrap_or(true)
        })
        .filter(|ms| {
            args.exclude_type
                .as_ref()
                .map(|vfstype| ms.mount.vfstype != *vfstype)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    if error_count == 0 {
        Ok(ms)
    } else {
        Err(Error::NotAccessible {
            accessible: ms,
            not_accessible: error_count,
        })
    }
}

fn do_check(mountstats: &[MountStat], args: &Args) -> Status {
    let mut status = Status::Ok;
    for ms in mountstats {
        let pcnt = percent(ms.stat.f_bavail, ms.stat.f_blocks);
        if pcnt > args.crit {
            status = Status::Critical;
            println!(
                "CRITICAL: {} has {:.1}% of its {}B used (> {:.1}%)",
                ms.mount.file,
                pcnt,
                bytes_to_human_size(ms.stat.f_blocks * ms.stat.f_frsize),
                args.crit
            )
        } else if pcnt > args.warn {
            status = max(status, Status::Warning);
            println!(
                "WARNING: {} has {:.1}% of its {}B used (> {:.1}%)",
                ms.mount.file,
                pcnt,
                bytes_to_human_size(ms.stat.f_blocks * ms.stat.f_frsize),
                args.warn
            )
        }

        let ipcnt = percent(ms.stat.f_favail, ms.stat.f_files);
        if ipcnt > args.crit_inodes {
            status = Status::Critical;
            println!(
                "CRITICAL: {} has {:.1}% of its {} inodes used (> {:.1}%)",
                ms.mount.file,
                ipcnt,
                bytes_to_human_size(ms.stat.f_files),
                args.crit_inodes
            );
        } else if ipcnt > args.warn_inodes {
            status = max(status, Status::Warning);
            println!(
                "WARNING: {} has {:.1}% of its {} inodes used (> {:.1}%)",
                ms.mount.file,
                ipcnt,
                bytes_to_human_size(ms.stat.f_files),
                args.warn_inodes
            );
        }
    }
    if status == Status::Ok {
        println!(
            "OKAY: {} filesystems checked, none are above {}% disk or {}% inode usage",
            mountstats.len(),
            args.warn,
            args.warn_inodes
        );
    }

    if args.info {
        println!(
            "{:<15} {:>7} {:>5}% {:>7} {:>5}% {:<20}",
            "Filesystem", "Size", "Use", "INodes", "IUse", "Mounted on"
        );
        for ms in mountstats {
            println!(
                "{:<15} {:>7} {:>5.1}% {:>7} {:>5.1}% {:<20}",
                ms.mount.spec,
                bytes_to_human_size(ms.stat.f_blocks * ms.stat.f_frsize),
                percent(ms.stat.f_bavail, ms.stat.f_blocks),
                bytes_to_human_size(ms.stat.f_files),
                percent(ms.stat.f_favail, ms.stat.f_files),
                ms.mount.file
            );
        }
    }

    status
}

#[cfg(test)]
mod unit {
    use super::{maybe_regex, Args};
    use structopt::StructOpt;

    #[test]
    fn validate_docstring() {
        let args: Args = Args::from_iter(["arg0", "--crit", "5"].into_iter());
        assert_eq!(args.crit, 5.0);
        let args: Args = Args::from_iter(["arg0", "--pattern", "hello"].into_iter());
        assert_eq!(args.pattern.unwrap(), "hello");
    }

    #[test]
    fn check_maybe_regex() {
        if let Err(emsg) = maybe_regex(&Some("[hello".to_owned())) {
            let expected = r#"Unable to filter disks like "[hello":"#;
            assert!(
                emsg.to_string().contains(expected),
                "\nExpected something containing: {}\n\
                 But instead received         : {}",
                expected,
                emsg.to_string()
            );
        } else {
            panic!("Should have gotten an error");
        }
    }
}

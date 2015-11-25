//! Check Disk usage

extern crate docopt;
extern crate nix;
extern crate regex;
extern crate rustc_serialize;

extern crate turbine_plugins;

use std::collections::HashSet;
use std::cmp::max;

use docopt::Docopt;
use regex::Regex;
use nix::sys::statvfs::vfs;
use turbine_plugins::Status;
use turbine_plugins::procfs::Mount;
use turbine_plugins::linux::bytes_to_human_size;

static USAGE: &'static str = "
Usage:
     check-disk [options] [thresholds] [filters]
     check-disk -h | --help

Check all mounted file systems for disk usage.

For some reason this check generally generates values that are between 1% and
3% higher than `df`, even though AFAICT we're both just calling statvfs a bunch
of times.

Options:
    -h, --help            Show this message and exit
    --info                Print information of all known filesystems.
                          Similar to df.

Thresholds:
    -w, --warn=<percent>  Percent usage to warn at. [default: 80]
    -c, --crit=<percent>  Percent usage to go critical at. [default: 90]
    -W, --warn-inodes=<percent>
                          Percent of inode usage to warn at. [default: 80]
    -C, --crit-inodes=<percent>
                          Percent of inode usage to go critical at. [default: 90]

Filters:
    --pattern=<regex>     Only check filesystems that match this regex.
    --exclude-pattern=<regex>  Do not check filesystems that match this regex.
    --type=<fs>           Only check filesystems that are of this type, e.g.
                          ext4 or tmpfs. See 'man 8 mount' for more examples.
    --exclude-type=<fs>   Do not check filesystems that are of this type.
";

#[derive(RustcDecodable)]
struct Args {
    flag_crit: f64,
    flag_warn: f64,
    flag_crit_inodes: f64,
    flag_warn_inodes: f64,
    flag_pattern: Option<String>,
    flag_exclude_pattern: Option<String>,
    flag_type: Option<String>,
    flag_exclude_type: Option<String>,
    flag_info: bool,
}

impl Args {
    fn parse() -> Args {
        Docopt::new(USAGE)
            .and_then(|d| d.decode())
            .unwrap_or_else(|e| e.exit())
    }
}

#[derive(Debug)]
struct MountStat {
    mount: Mount,
    stat: vfs::Statvfs,
}

fn percent(part: u64, whole: u64) -> f64 {
    100.0 - (part as f64 / whole as f64) * 100.0
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
fn filter(mounts: Vec<Mount>, args: &Args) -> Vec<MountStat> {
    let mut devices = HashSet::new();
    mounts.into_iter()
        .filter_map(|mount| {
            let stat = vfs::Statvfs::for_path(mount.file.as_bytes()).unwrap();
            if stat.f_blocks > 0 && !mount.file.starts_with("/proc") {
                Some(MountStat {
                    mount: mount,
                    stat: stat,
                })
            } else {
                None
            }
        })
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
        .filter(|ms|
                if let Some(ref pattern) = args.flag_pattern {
                    let re = Regex::new(&pattern).unwrap();
                    re.is_match(&ms.mount.file)
                } else {
                    true
                })
        .filter(|ms|
                if let Some(ref pattern) = args.flag_exclude_pattern {
                    let re = Regex::new(&pattern).unwrap();
                    !re.is_match(&ms.mount.file)
                } else {
                    true
                })
        .filter(|ms|
                if let Some(ref vfstype) = args.flag_type {
                    ms.mount.vfstype == *vfstype
                } else {
                    true
                })
        .filter(|ms|
                if let Some(ref vfstype) = args.flag_exclude_type {
                    ms.mount.vfstype != *vfstype
                } else {
                    true
                })
        .collect::<Vec<_>>()
}

fn do_check(mountstats: &[MountStat], args: &Args) -> Status {
    let mut status = Status::Ok;
    for ms in mountstats {
        let pcnt = percent(ms.stat.f_bavail, ms.stat.f_blocks);
        if pcnt > args.flag_crit {
            status = Status::Critical;
            println!("CRITICAL: {} has {:.1}% of its {}B used (> {:.1}%)",
                     ms.mount.file,
                     pcnt,
                     bytes_to_human_size(ms.stat.f_blocks * ms.stat.f_frsize),
                     args.flag_crit)
        } else if pcnt > args.flag_warn {
            status = max(status, Status::Warning);
            println!("WARNING: {} has {:.1}% of its {}B used (> {:.1}%)",
                     ms.mount.file,
                     pcnt,
                     bytes_to_human_size(ms.stat.f_blocks * ms.stat.f_frsize),
                     args.flag_warn)
        }

        let ipcnt = percent(ms.stat.f_favail, ms.stat.f_files);
        if ipcnt > args.flag_crit_inodes {
            status = Status::Critical;
            println!("CRITICAL: {} has {:.1}% of its {} inodes used (> {:.1}%)",
                     ms.mount.file,
                     ipcnt,
                     bytes_to_human_size(ms.stat.f_files),
                     args.flag_crit_inodes);
        } else if ipcnt > args.flag_warn_inodes {
            status = max(status, Status::Warning);
            println!("WARNING: {} has {:.1}% of its {} inodes used (> {:.1}%)",
                     ms.mount.file,
                     ipcnt,
                     bytes_to_human_size(ms.stat.f_files),
                     args.flag_warn_inodes);
        }
    }
    if status == Status::Ok {
        println!("OKAY: {} filesystems checked, none are above {}% disk or {}% inode usage",
                 mountstats.len(), args.flag_warn, args.flag_warn_inodes);
    }

    if args.flag_info {
        println!("{:<15} {:>7} {:>5}% {:>7} {:>5}% {:<20}",
                 "Filesystem", "Size", "Use", "INodes", "IUse", "Mounted on");
        for ms in mountstats {
            println!("{:<15} {:>7} {:>5.1}% {:>7} {:>5.1}% {:<20}",
                     ms.mount.spec,
                     bytes_to_human_size(ms.stat.f_blocks * ms.stat.f_frsize),
                     percent(ms.stat.f_bavail, ms.stat.f_blocks),
                     bytes_to_human_size(ms.stat.f_files),
                     percent(ms.stat.f_favail, ms.stat.f_files),
                     ms.mount.file);
        }
    }

    status
}

fn main() {
    let args = Args::parse();

    let mut mounts = Mount::load_all().unwrap();
    mounts.sort_by(|l, r| l.file.len().cmp(&r.file.len()));

    let mountstats = filter(mounts, &args);

    let status = do_check(&mountstats, &args);
    status.exit();
}

#[cfg(test)]
mod unit {
    use super::{Args, USAGE};
    use docopt::Docopt;

    #[test]
    fn validate_docstring() {
        let args: Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(vec!["arg0", "--crit", "5"].into_iter())
                      .decode())
            .unwrap();
        assert_eq!(args.flag_crit, 5.0);
        let args: Args = Docopt::new(USAGE)
            .and_then(|d| d.argv(vec!["arg0", "--pattern", "hello"].into_iter()).decode())
            .unwrap();
        assert_eq!(args.flag_pattern.unwrap(), "hello");
    }
}

// Fake module for documentation
//! Documentation about the various scripts contained herein
//!
//! - [check-graphite](#check-graphite)
//! - [check-cpu](#check-cpu)
//! - [check-fs-writeable](#check-fs-writeable)
//! - [check-load](#check-load)
//! - [check-ram](#check-ram)
//!
//! check-graphite
//! ==============
//!
//! Usage looks like:
//!
//! ```plain
//! $ check-graphite -h
//! check-graphite 0.1.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Query graphite and exit based on predicates
//!
//! USAGE:
//!         check-graphite [FLAGS] [OPTIONS] <URL> <PATH> <ASSERTION>
//!
//! FLAGS:
//!     -h, --help       Prints help information
//!     -V, --version    Prints version information
//!
//! OPTIONS:
//!         --no-data <NO_DATA_STATUS>    What to do with no data. Choices: ok, warn, critical, unknown. Default: warn. [values: critical ok unknown warn]
//!     -w, --window <WINDOW>             How many minutes of data to test. Default 10.
//!
//! ARGS:
//!     URL          The domain to query graphite. Must include scheme (http/s)
//!     PATH         The graphite path to query. For example: "collectd.*.cpu"
//!     ASSERTION    The assertion to make against the PATH. See Below.
//!
//! About Assertions:
//!
//!     Assertions look like "critical if any point in any series is > 5".
//!
//!     They describe what you care about in your graphite data. The structure of
//!     an assertion is as follows:
//!
//!         <errorkind> if <point spec> [in <series spec>] is|are [not] <operator> <threshold>
//!
//!     Where:
//!
//!     - `errorkind` is either `critical` or `warning`
//!     - `point spec` can be one of:
//!         - `any point`
//!         - `all points`
//!         - `at least <N>% of points`
//!         - `most recent point`
//!     - `series spec` (optional) can be one of:
//!         - `any series`
//!         - `all series`
//!         - `at least <N>% of series`
//!     - `not` (optional) inverts the following operator
//!     - `operator` is one of: `==` `!=` `<` `>` `<=` `>=`
//!     - `threshold` is a floating-point value (e.g. 100, 78.0)
//!
//!     Here are some example assertions:
//!
//!     - `critical if any point is > 0`
//!     - `warning if any point is == 9`
//!     - `critical if all points are > 100.0`
//!     - `critical if at least 20% of points are > 100`
//!     - `critical if most recent point is > 5`
//!     - `critical if most recent point in all series == 0`
//! ```
//!
//! check-cpu
//! =========
//!
//! ```plain
//! $ check-cpu -h
//! Usage: check-cpu [options] [--type=<work-source>...] [--show-hogs=<count>]
//!
//! Options:
//!     -h, --help               Show this help message
//!
//!     -s, --sample=<seconds>   Seconds to spent collecting   [default: 1]
//!     -w, --warn=<percent>     Percent to warn at            [default: 80]
//!     -c, --crit=<percent>     Percent to critical at        [default: 95]
//!     --show-hogs=<count>      Show most cpu-hungry procs    [default: 0]
//!
//! CPU Work Types:
//!
//!     Specifying one of the CPU kinds checks that kind of utilization. The
//!     default is to check total utilization. Specifying this multiple times
//!     alerts if *any* of the CPU usage types are critical.
//!
//!     --type=<usage>           Some of:
//!                                 total user nice system idle
//!                                 iowait irq softirq steal guest [default: total]
//! ```
//!
//! check-fs-writeable
//! ==================
//!
//! ```plain
//! $ check-fs-writeable -h
//! Usage:
//!     check-fs-writeable <filename>
//!     check-fs-writeable -h | --help
//!
//! Check that we can write to a filesystem by writing a byte to a file. Does not
//! try to create the directory, or do anything else. Just writes a single byte to
//! a file.
//!
//! Arguments:
//!
//!     <filename>            The file to write to
//!
//! Options:
//!     -h, --help            Show this message and exit
//! ```
//!
//! check-load
//! ==========
//!
//! Linux-only.
//!
//! ```plain
//! $ check-load -h
//! Usage: check-load [options]
//!        check-load -h | --help
//!
//! Check the load average of the system
//!
//! Load average is the number of processes *waiting* to do work in a queue, either
//! due to IO or CPU constraints. The numbers used to check are the load averaged
//! over 1, 5 and 15 minutes, respectively
//!
//! Options:
//!     -h, --help              Show this message and exit
//!     -v, --verbose           Print even when things are okay
//!
//! Threshold Behavior:
//!     -w, --warn=<averages>   Averages to warn at         [default: 5,3.5,2.5]
//!     -c, --crit=<averages>   Averages to go critical at  [default: 10,5,3]
//!
//!     --per-cpu=<maybe>      Divide the load average by the number of processors on the
//!                            system.                      [default: true]
//! ```
//!
//! check-ram
//! =========
//!
//! Linux-only.
//!
//! ```plain
//! $ check-ram -h
//! Usage: check-ram [options]
//!        check-ram -h | --help
//!
//! Options:
//!     -h, --help             Show this help message
//!
//!     -w, --warn=<percent>   Percent used to warn at      [default: 80]
//!     -c, --crit=<percent>   Percent used to critical at  [default: 95]
//!
//!     --show-hogs=<count>    Show most RAM-hungry procs   [default: 0]
//!     -v, --verbose          Always show the hogs
//! ```
//!
//! check-disk
//! ==========
//!
//! Linux-only.
//!
//! ```plain
//! $ check-disk -h
//! Usage:
//!      check-disk [options] [thresholds] [filters]
//!      check-disk -h | --help
//!
//! Check all mounted file systems for disk usage.
//!
//! For some reason this check generally generates values that are between 1% and
//! 3% higher than `df`, even though AFAICT we're both just calling statvfs a bunch
//! of times.
//!
//! Options:
//!     -h, --help            Show this message and exit
//!     --info                Print information of all known filesystems.
//!                           Similar to df.
//!
//! Thresholds:
//!     -w, --warn=<percent>  Percent usage to warn at. [default: 80]
//!     -c, --crit=<percent>  Percent usage to go critical at. [default: 90]
//!     -W, --warn-inodes=<percent>
//!                           Percent of inode usage to warn at. [default: 80]
//!     -C, --crit-inodes=<percent>
//!                           Percent of inode usage to go critical at. [default: 90]
//!
//! Filters:
//!     --pattern=<regex>     Only check filesystems that match this regex.
//!     --exclude-pattern=<regex>  Do not check filesystems that match this regex.
//!     --type=<fs>           Only check filesystems that are of this type, e.g.
//!                           ext4 or tmpfs. See 'man 8 mount' for more examples.
//!     --exclude-type=<fs>   Do not check filesystems that are of this type.
//! ```

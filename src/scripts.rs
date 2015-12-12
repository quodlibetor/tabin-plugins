// Fake module for documentation
//! Documentation about the various scripts contained herein
//!
//! - [check-graphite](#check-graphite)
//! - [check-cpu](#check-cpu)
//! - [check-container-cpu](#check-container-cpu)
//! - [check-load](#check-load)
//! - [check-container-ram](#check-container-ram)
//! - [check-ram](#check-ram)
//! - [check-fs-writeable](#check-fs-writeable)
//! - [check-disk](#check-disk)
//!
//! check-graphite
//! ==============
//!
//! Usage looks like:
//!
//! ```plain
//! $ check-graphite --help
//! check-graphite 0.1.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Query graphite and exit based on predicates
//!
//! USAGE:
//!         check-graphite [FLAGS] [OPTIONS] <URL> <PATH> <ASSERTION>... [--]
//!
//! FLAGS:
//!     -h, --help                 Prints help information
//!         --print-url            Unconditionally print the graphite url queried
//!     -V, --version              Prints version information
//!         --verify-assertions    Just check assertion syntax, do not query urls
//!
//! OPTIONS:
//!         --retries <COUNT>      How many times to retry reaching graphite. Default 4
//!         --graphite-error <GRAPHITE_ERROR_STATUS>    What to do with no data.
//!                               Choices: ok, warn, critical, unknown.
//!                               What to say if graphite returns a 500 or invalid JSON
//!                               Default: unknown. [values: ok warning critical unknown]
//!     -w, --window <MINUTES>                          How many minutes of data to test. Default 10.
//!         --no-data <NO_DATA_STATUS>                  What to do with no data.
//!                               Choices: ok, warn, critical, unknown.
//!                               This is the value to use for the assertion
//!                               'if all values are null'
//!                               Default: warn. [values: ok warning critical unknown]
//!
//! ARGS:
//!     URL             The domain to query graphite. Must include scheme (http/s)
//!     PATH            The graphite path to query. For example: "collectd.*.cpu"
//!     ASSERTION...    The assertion to make against the PATH. See Below.
//!
//! About Assertions:
//!
//!     Assertions look like 'critical if any point in any series is > 5'.
//!
//!     They describe what you care about in your graphite data. The structure of
//!     an assertion is as follows:
//!
//!         <errorkind> if <point spec> [in <series spec>] is|are [not] <operator> <threshold>
//!
//!     Where:
//!
//!         - `errorkind` is either `critical` or `warning`
//!         - `point spec` can be one of:
//!             - `any point`
//!             - `all points`
//!             - `at least <N>% of points`
//!             - `most recent point`
//!         - `series spec` (optional) can be one of:
//!             - `any series`
//!             - `all series`
//!             - `at least <N>% of series`
//!         - `not` is optional, and inverts the following operator
//!         - `operator` is one of: `==` `!=` `<` `>` `<=` `>=`
//!         - `threshold` is a floating-point value (e.g. 100, 78.0)
//!
//!     Here are some example assertions:
//!
//!         - `critical if any point is > 0`
//!         - `critical if any point in at least 40% of series is > 0`
//!         - `critical if any point is not > 0`
//!         - `warning if any point is == 9`
//!         - `critical if all points are > 100.0`
//!         - `critical if at least 20% of points are > 100`
//!         - `critical if most recent point is > 5`
//!         - `critical if most recent point in all series are == 0`
//! ```
//!
//! check-cpu
//! =========
//!
//! Linux-only.
//!
//! ```plain
//! Usage:
//!     check-cpu [options] [--type=<work-source>...] [--show-hogs=<count>]
//!     check-cpu (-h | --help)
//!
//! Options:
//!     -h, --help               Show this help message
//!
//!     -s, --sample=<seconds>   Seconds to spent collecting   [default: 1]
//!     -w, --warn=<percent>     Percent to warn at            [default: 80]
//!     -c, --crit=<percent>     Percent to critical at        [default: 95]
//!
//!     --per-cpu                Gauge values per-cpu instead of across the
//!                              entire machine
//!     --cpu-count=<num>        If --per-cpu is specified, this is how many
//!                              CPUs need to be at a threshold to trigger.
//!                              [default: 1]
//!
//!     --show-hogs=<count>      Show most cpu-hungry procs    [default: 0]
//!
//! CPU Work Types:
//!
//!     Specifying one of the CPU kinds checks that kind of utilization. The
//!     default is to check total utilization. Specifying this multiple times
//!     alerts if *any* of the CPU usage types are critical.
//!
//!     There are three CPU type groups: `active` `activeplusiowait` and
//!     `activeminusnice`. `activeplusiowait` considers time spent waiting for IO
//!     to be busy time, this gets alerts to be more aligned with the overall
//!     system load, but is different from CPU usage reported by `top` since the
//!     CPU isn't actually *busy* during this time.
//!
//!     --type=<usage>           Some of:
//!                                 active activeplusiowait activeminusnice
//!                                 user nice system irq softirq steal guest
//!                                 idle iowait [default: active]
//! ```
//!
//! check-container-cpu
//! ===================
//!
//! Linux-only. Can only be run from inside a cgroup.
//!
//! ```plain
//! $ target/osx/debug/check-container-cpu -h
//! Usage:
//!     check-container-cpu [options]
//!     check-cpu (-h | --help)
//!
//! Check the cpu usage of the currently-running container. This must be run from
//! inside the container to be checked.
//!
//! Options:
//!     -h, --help            Show this help message
//!
//!     -w, --warn=<percent>  Percent to warn at           [default: 80]
//!     -c, --crit=<percent>  Percent to critical at       [default: 80]
//!
//!     -s, --sample=<secs>   Seconds to take sample over  [default: 5]
//!
//!     --show-hogs=<count>   Show <count> most cpu-intensive processes in this
//!                           container.                   [default: 0]
//!
//!     --shares-per-cpu=<shares>
//!                           The number of CPU shares given to a cgroup when
//!                           it has exactly one CPU allocated to it.
//!
//! About usage percentages:
//!
//!     If you don't specify '--shares-per-cpu', percentages should be specified
//!     relative to a single CPU's usage. So if you have a process that you want to
//!     be allowed to use 4 CPUs worth of processor time, and you were planning on
//!     going critical at 90%, you should specify something like '--crit 360'
//!
//!     However, if you are using a container orchestrator such as Mesos, you often
//!     tell it that you want this container to have "2 CPUs" worth of hardware.
//!     Your scheduler is responsible for deciding how many cgroup cpu shares 1
//!     CPU's worth of time is, and keeping track of how many shares it has doled
//!     out, and then schedule your containers to run with 2 CPUs worth of CPU
//!     shares. Assuming that your scheduler uses the default number of shares
//!     (1024) as "one cpu", this will mean that you have given that cgroup 2048
//!     shares.
//!
//!     If you do specify --shares-per-cpu then the percentage that you give will
//!     be scaled by the number of CPUs worth of shares that this container has
//!     been given, and CPU usage will be compared to the total percent of the CPUs
//!     that it has been allocated.
//!
//!     Which is to say, if you specify --shares-per-cpu, you should always specify
//!     your warn/crit percentages out of 100%, because this script will correctly
//!     scale it for your process.
//!
//!     Here are some examples, where 'shares granted' is the value in
//!     /sys/fs/cgroup/cpu/cpu.shares:
//!
//!         * args: --shares-per-cpu 1024 --crit 90
//!           shares granted: 1024
//!           percent of one CPU to alert at: 90
//!         * args: --shares-per-cpu 1024 --crit 90
//!           shares granted: 2024
//!           percent of one CPU to alert at: 180
//!         * args: --shares-per-cpu 1024 --crit 90
//!           shares granted: 102
//!           percent of one CPU to alert at: 9
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
//!     --per-cpu               Divide the load average by the number of processors on the
//!                             system.
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
//! check-container-ram
//! ===================
//!
//! Linux-only. Can only be run from inside a container.
//!
//! ```plain
//! $ target/osx/debug/check-container-ram -h
//! Usage:
//!     check-container-ram [--show-hogs=<count>] [--invalid-limit=<status>] [options]
//!     check-container-ram (-h | --help)
//!
//! Check the RAM usage of the currently-running container. This must be run from
//! inside the container to be checked.
//!
//! This checks as a ratio of the limit specified in the cgroup memory limit, and
//! if there is no limit set (or the limit is greater than the total memory
//! available on the system) this checks against the total system memory.
//!
//! Options:
//!     -h, --help                 Show this message and exit
//!
//!     -w, --warn=<percent>       Warn at this percent used           [default: 85]
//!     -c, --crit=<percent>       Critical at this percent used       [default: 95]
//!
//!     --invalid-limit=<status>   Status to consider this check if the CGroup limit
//!                                is greater than the system ram      [default: ok]
//!
//!     --show-hogs=<count>        Show the most ram-hungry procs      [default: 0]
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

//! Documentation about the various scripts contained herein
//!
//! - [check-graphite](#check-graphite)
//! - [check-cpu](#check-cpu)
//! - [check-container-cpu](#check-container-cpu)
//! - [check-load](#check-load)
//! - [check-ram](#check-ram)
//! - [check-container-ram](#check-container-ram)
//! - [check-procs](#check-procs)
//! - [check-fs-writeable](#check-fs-writeable)
//! - [check-disk](#check-disk)
//!
//! # check-graphite
//!
//! Cross platform, only requires access to a graphite instance.
//!
//! ```plain
//! $ check-graphite --help
//! check-graphite (part of tabin-plugins) 0.3.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Query graphite and exit based on predicates
//!
//! USAGE:
//!     check-graphite [FLAGS] [OPTIONS] <URL> <PATH> <ASSERTION>...
//!
//! FLAGS:
//!     -h, --help                 Prints help information
//!         --print-url            Unconditionally print the graphite url queried
//!     -V, --version              Prints version information
//!         --verify-assertions    Just check assertion syntax, do not query urls
//!
//! OPTIONS:
//!         --graphite-error <GRAPHITE_ERROR_STATUS>
//!             What to say if graphite returns a 500 or invalid JSON. Default: unknown. [possible values: ok, warning,
//!             critical, unknown]
//!         --no-data <NO_DATA_STATUS>
//!             What to do with no data. This is the value to use for the assertion 'if all values are null' Default: warn.
//!             [possible values: ok, warning, critical, unknown]
//!         --retries <COUNT>                           How many times to retry reaching graphite. Default 4.
//!     -w, --window <MINUTES>                          How many minutes of data to test. Default 10.
//!         --window-start <MINUTES_IN_PAST>            How far back to start the window. Default is now.
//!
//! ARGS:
//!     <URL>             The domain to query graphite. Must include scheme (http/s)
//!     <PATH>            The graphite path to query. For example: "collectd.*.cpu"
//!     <ASSERTION>...    The assertion to make against the PATH. See Below.
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
//!             - `not` is optional, and inverts the following operator
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
//!
//! ```
//!
//! # check-cpu
//!
//! Linux-only.
//!
//! ```plain
//! $ check-cpu --help
//! check-cpu  (part of tabin-plugins) 0.3.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Check cpu usage of the current computer
//!
//! USAGE:
//!     check-cpu [FLAGS] [OPTIONS]
//!
//! FLAGS:
//!     -h, --help       Prints help information
//!         --per-cpu    Gauge values per-cpu instead of across the entire machine
//!     -V, --version    Prints version information
//!
//! OPTIONS:
//!         --show-hogs <count>        Show <count> most cpu-intensive processes in this container. [default: 0]
//!         --cpu-count <cpu_count>    If --per-cpu is specified, this is how many
//!                                                                 CPUs need to be at a threshold to trigger. [default: 1]
//!     -c, --crit <crit>              Percent to go critical at [default: 80]
//!     -s, --sample <seconds>         Seconds to take sample over [default: 5]
//!     -w, --warn <warn>              Percent to warn at [default: 80]
//!         --type <work_type>...      See 'CPU Work Types, below [default: active]
//!
//! CPU Work Types:
//!
//!     Specifying one of the CPU kinds via `--type` checks that kind of
//!     utilization. The default is to check total utilization. Specifying this
//!     multiple times alerts if *any* of the CPU usage types are critical.
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
//!
//! ```
//!
//! # check-container-cpu
//!
//! Linux-only. Can only be run from inside a cgroup.
//!
//! ```plain
//! $ check-container-cpu --help
//! check-container-cpu (part of tabin-plugins) 0.3.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Check the cpu usage of the currently-running container.
//!
//! This must be run from inside the container to be checked.
//!
//! USAGE:
//!     check-container-cpu [OPTIONS]
//!
//! FLAGS:
//!     -h, --help       Prints help information
//!     -V, --version    Prints version information
//!
//! OPTIONS:
//!         --show-hogs <count>                  Show <count> most cpu-intensive processes in this container. [default: 0]
//!     -c, --crit <crit>                        Percent to go critical at [default: 80]
//!     -s, --sample <seconds>                   Seconds to take sample over [default: 5]
//!         --shares-per-cpu <shares_per_cpu>    The number of CPU shares given to a cgroup when it has exactly one CPU
//!                                              allocated to it.
//!     -w, --warn <warn>                        Percent to warn at [default: 80]
//!
//! About usage percentages:
//!
//!     If you don't specify '--shares-per-cpu', percentages should be specified
//!     relative to a single CPU's usage. So if you have a process that you want to
//!     be allowed to use 4 CPUs worth of processor time, and you were planning on
//!     going critical at 90%, you should specify something like '--crit 360'
//!
//!     However, if you are using a container orchestrator such as Mesos, you often
//!     tell it that you want this container to have '2 CPUs' worth of hardware.
//!     Your scheduler is responsible for deciding how many cgroup cpu shares 1
//!     CPU's worth of time is, and keeping track of how many shares it has doled
//!     out, and then schedule your containers to run with 2 CPUs worth of CPU
//!     shares. Assuming that your scheduler uses the default number of shares
//!     (1024) as 'one cpu', this will mean that you have given that cgroup 2048
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
//!
//! ```
//!
//! # check-load
//!
//! Linux-only.
//!
//! ```plain
//! $ check-load --help
//! check-load (part of tabin-plugins) 0.3.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Check the load average of the system
//!
//! Load average is the number of processes *waiting* to do work in a queue, either due to IO or CPU constraints. The
//! numbers used to check are the load averaged over 1, 5 and 15 minutes, respectively
//!
//! USAGE:
//!     check-load [FLAGS] [OPTIONS]
//!
//! FLAGS:
//!     -h, --help       Prints help information
//!         --per-cpu    Divide the load average by the number of processors on the system.
//!     -V, --version    Prints version information
//!     -v, --verbose    print info even if everything is okay
//!
//! OPTIONS:
//!     -c, --crit <crit>    Averages to go critical at [default: 10,5,3]
//!     -w, --warn <warn>    Averages to warn at [default: 5,3.5,2.5]
//!
//! ```
//!
//! # check-ram
//!
//! Linux-only.
//!
//! ```plain
//! $ check-ram --help
//! check-ram (part of tabin-plugins) 0.3.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Check the ram usage of the current computer
//!
//! USAGE:
//!     check-ram [OPTIONS]
//!
//! FLAGS:
//!     -h, --help       Prints help information
//!     -V, --version    Prints version information
//!
//! OPTIONS:
//!         --show-hogs <count>    Show <count> most ram-intensive processes in this computer. [default: 0]
//!     -c, --crit <crit>          Percent to go critical at [default: 95]
//!     -w, --warn <warn>          Percent to warn at [default: 85]
//!
//! ```
//!
//! # check-container-ram
//!
//! Linux-only. Can only be run from inside a cgroup.
//!
//! ```plain
//! $ check-container-ram --help
//! check-container-ram (part of tabin-plugins) 0.3.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Check the RAM usage of the currently-running container.
//!
//! This must be run from inside the container to be checked.
//!
//! This checks as a ratio of the limit specified in the cgroup memory limit, and if there is no limit set (or the limit is
//! greater than the total memory available on the system) this checks against the total system memory.
//!
//! USAGE:
//!     check-container-ram [OPTIONS]
//!
//! FLAGS:
//!     -h, --help       Prints help information
//!     -V, --version    Prints version information
//!
//! OPTIONS:
//!         --show-hogs <count>                Show <count> most ram-intensive processes in this container. [default: 0]
//!     -c, --crit <crit>                      Percent to go critical at [default: 95]
//!         --invalid-limit <invalid_limit>    Status to consider this check if the CGroup limit is greater than the system
//!                                            ram [default: ok]
//!     -w, --warn <warn>                      Percent to warn at [default: 85]
//!
//! ```
//!
//! # check-procs
//!
//! Linux-only. Reads running processes
//!
//! ```plain
//! $ check-procs --help
//! check-procs (part of tabin-plugins) 0.3.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Check that an expected number of processes are running.
//!
//! Optionally, kill unwanted processes.
//!
//! USAGE:
//!     check-procs [FLAGS] [OPTIONS] [--] [pattern]
//!
//! FLAGS:
//!         --allow-unparseable-procs    In combination with --crit-over M this will not alert if any processes cannot be
//!                                      parsed
//!     -h, --help                       Prints help information
//!     -V, --version                    Prints version information
//!
//! OPTIONS:
//!         --crit-over <M>                               Error if there are more than <M> procs matching <pattern>
//!         --crit-under <N>                              Error if there are fewer than <N> procs matching <pattern>
//!         --kill-parents-of-matching <PARENT_SIGNAL>
//!             If *any* processes match, then kill their parents with the provided signal which can be either an integer or
//!             a name like KILL or SIGTERM. This has the same exit status behavior as kill-matching.
//!         --kill-matching <SIGNAL>
//!             If *any* processes match, then kill them with the provided signal which can be either an integer or a name
//!             like KILL or SIGTERM. This option does not affect the exit status, all matches are always killed, and if
//!             --crit-under/over are violated then then this will still exit critical.
//!         --state <states>...
//!             Filter to only processes in these states. If passed multiple times, processes matching any state are
//!             included.
//!             Choices: running sleeping uninterruptible-sleep waiting stopped zombie
//!
//! ARGS:
//!     <pattern>    Regex that command and its arguments must match
//!
//! Examples:
//!
//!     Ensure at least two nginx processes are running:
//!
//!         check-procs --crit-under 2 nginx
//!
//!     Ensure there are not more than 30 zombie proccesses on the system:
//!
//!         check-procs --crit-over 30 --state zombie
//!
//!     Ensure that there are not more than 5 java processes running MyMainClass
//!     that are in the zombie *or* waiting states:
//!
//!         check-procs --crit-over 5 --state zombie --state waiting 'java.*MyMainClass'
//!
//!     Ensure that there are at least three (running or waiting) (cassandra or
//!     postgres) processes:
//!
//!         check-procs --crit-under 3 --state running --state waiting 'cassandra|postgres'
//!
//! ```
//!
//! # check-fs-writeable
//!
//! 
//!
//! ```plain
//! $ check-fs-writeable --help
//! check-fs-writeable (part of tabin-plugins) 0.3.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Check that we can write to a filesystem by writing a byte to a file.
//!
//! Does not try to create the directory, or do anything else. Just writes a single byte to a file, errors if it cannot, and
//! then deletes the file.
//!
//! USAGE:
//!     check-fs-writeable <filename>
//!
//! FLAGS:
//!     -h, --help       Prints help information
//!     -V, --version    Prints version information
//!
//! ARGS:
//!     <filename>    The file to write to
//!
//! ```
//!
//! # check-disk
//!
//! Unix only.
//!
//! ```plain
//! $ check-disk --help
//! check-disk (part of tabin-plugins) 0.3.0
//! Brandon W Maister <quodlibetor@gmail.com>
//! Check all mounted file systems for disk usage.
//!
//! For some reason this check generally generates values that are between 1% and 3% higher than `df`, even though AFAICT
//! we're both just calling statvfs a bunch of times.
//!
//! USAGE:
//!     check-disk [FLAGS] [OPTIONS]
//!
//! FLAGS:
//!     -h, --help       Prints help information
//!         --info       Print information of all known filesystems. Similar to df.
//!     -V, --version    Prints version information
//!
//! OPTIONS:
//!     -c, --crit <crit>                        Percent to go critical at [default: 90]
//!     -C, --crit-inodes <crit_inodes>          Percent of inode usage to go critical at [default: 90]
//!         --exclude-type <exclude-fs-type>     Do not check filesystems that are of this type.
//!         --exclude-pattern <exclude-regex>    Only check filesystems that match this regex
//!         --type <fs-type>                     Only check filesystems that are of this type, e.g. ext4 or tmpfs. See 'man
//!                                              8 mount' for more examples.
//!         --pattern <regex>                    Only check filesystems that match this regex
//!     -w, --warn <warn>                        Percent to warn at [default: 80]
//!     -W, --warn-inodes <warn_inodes>          Percent of inode usage to warn at [default: 80]
//!
//! ```


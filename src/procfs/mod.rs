//! Structs and impls for the various files from the /proc filesystem
//!
//! Each file gets a struct to represent its data, with an associated `load`
//! function.

use std::collections::{hash_map, HashMap};
use std::fs::{self, File};
use std::io::{self, Read};
use std::ops::{Div, Sub};
use std::result::Result as StdResult;
use std::str::{FromStr, Split};
use std::fmt;
use std::num;

use regex::Regex;
use std::slice;

use linux::Jiffies;
use procfs::pid::ProcessCpuUsage;

pub mod pid;

/// ProcFs errors
///
/// Every error from in this module can be converted into a `ProcFsError`
#[derive(Debug, From)]
pub enum ProcFsError {
    /// Errors originating in IO
    Io(io::Error),
    /// Error pulling all required data out of procfs
    InsufficientData(String),
    /// Happens when we try to parse a float from something in procfs
    InvalidFloat(num::ParseFloatError),
    /// Happens when we try to parse an int from something in procfs
    InvalidInt(num::ParseIntError),
    /// When we receive an error loading an individual proc, we get this
    LoadProcsError(LoadProcsError),
    /// Happens when we try to parse a line from /proc/<pid>/stat and got an error
    ParseStatError(ParseStatError),
    /// Happens when we get an invalid process state
    ParseStateError(ParseStateError),
}

impl fmt::Display for ProcFsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> StdResult<(), fmt::Error> {
        use self::ProcFsError::*;
        match self {
            &Io(ref e) => write!(f, "{}", e),
            &InsufficientData(ref e) => write!(f, "{}", e),
            &InvalidFloat(ref e) => write!(f, "{}", e),
            &InvalidInt(ref e) => write!(f, "{}", e),
            &LoadProcsError(ref e) => write!(f, "{}", e),
            &ParseStatError(ref e) => write!(f, "{}", e),
            &ParseStateError(ref e) => write!(f, "{}", e),
        }
    }
}

#[derive(Debug)]
pub struct LoadProcsError {
    /// The data we successfully loaded
    pub procs: RunningProcs,
    /// The errors we got when loading
    pub errors: Vec<ProcFsError>,
}

impl fmt::Display for LoadProcsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> StdResult<(), fmt::Error> {
        write!(
            f,
            "loaded {} processes correctly, but got {} errors:\n",
            self.procs.len(),
            self.errors.len()
        )?;
        for err in &self.errors {
            write!(f, "    {}", err)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ParseStatError {
    pub line: String,
    pub field_name: &'static str,
    pub position: u8,
}

impl fmt::Display for ParseStatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> StdResult<(), fmt::Error> {
        write!(
            f,
            "unable to parse '{}' at position {} from line '{}'",
            self.field_name, self.position, self.line
        )
    }
}

#[derive(Debug)]
pub struct ParseStateError {
    state: String,
}

impl fmt::Display for ParseStateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> StdResult<(), fmt::Error> {
        write!(
            f,
            "String '{}' was not a valid state, expected one of \
             R,running, S,sleeping, D,uninteruptible-sleep, W,waiting, T,stopped, Z,zombie",
            self.state
        )
    }
}

/// All the results are results with `ProcFsError`s
pub type Result<T> = StdResult<T, ProcFsError>;

/// A collection of `ProcessCpuUsage`s
///
/// This is basically the value of the `RunningProcs::currently_running()` map.
pub struct ProcessCpuUsages<'a>(Vec<ProcessCpuUsage<'a>>);

pub enum ProcField {
    /// Total (system + user) CPU usage
    TotalCpu,
}

impl<'a> ProcessCpuUsages<'a> {
    /// Sort the processes by the field
    ///
    /// See the `ProcField` docs for details
    pub fn sort_by_field(&mut self, field: ProcField) {
        match field {
            ProcField::TotalCpu => self.0
                .sort_by(|l, r| r.total.partial_cmp(&l.total).unwrap()),
        }
    }

    /// The number of processes we successfully loaded
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Get an iterator over the underlying Vec.
    pub fn iter(&'a self) -> slice::Iter<'a, ProcessCpuUsage<'a>> {
        self.0.iter()
    }
}

pub type ProcMap = HashMap<i32, pid::Process>;
/// All the processes that are running
// TODO: make this internal field private, and re-export the methods
// on the vec.
#[derive(Debug)]
pub struct RunningProcs(pub ProcMap);

impl RunningProcs {
    /// Load the currently running processes from /proc/[pid]/*
    pub fn currently_running() -> Result<RunningProcs> {
        let mut procs = ProcMap::new();
        let mut errors = vec![];
        let is_digit = Regex::new(r"^[0-9]+$").unwrap();
        for entry in try!(fs::read_dir("/proc")) {
            if entry.is_err() {
                continue;
            }
            let entry = entry.unwrap();
            if let Some(fname) = entry.path().file_name() {
                fname
                    .to_str()
                    .map(|fname| {
                        if !is_digit.is_match(fname) {
                            // we only care about the pid files
                            return;
                        }
                        match pid::Process::from_pid(fname) {
                            Ok(prc) => {
                                procs.insert(prc.stat.pid, prc);
                            }
                            Err(e) => errors.push(e),
                        }
                    })
                    .unwrap(); // all /proc filenames should be legal
            }
        }
        if errors.is_empty() {
            Ok(RunningProcs(procs))
        } else {
            Err(LoadProcsError {
                procs: RunningProcs(procs),
                errors,
            }.into())
        }
    }

    pub fn empty() -> RunningProcs {
        RunningProcs(HashMap::new())
    }

    fn iter(&self) -> hash_map::Iter<i32, pid::Process> {
        self.0.iter()
    }

    /// Collect CPU usage for each proc as compared to the total active CPU
    /// time for the system.
    ///
    /// The value for `total_cpu` should probably be the result of subtracting
    /// two `Calculations::total()`s from each other.
    pub fn percent_cpu_util_since<'a>(
        &'a self,
        start: &'a RunningProcs,
        total_cpu: Jiffies,
    ) -> ProcessCpuUsages<'a> {
        let me = &self.0;
        let mut usages = Vec::new();
        for (_start_pid, start_process) in start.iter() {
            if let Some(end_process) = me.get(&start_process.stat.pid) {
                usages.push(end_process.cpu_utilization_since(start_process, total_cpu));
            }
        }
        ProcessCpuUsages(usages)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

// ////////////////////////////////////////////////////////////////////////////
// System-Level totals

/// A kind of CPU usage
///
/// Corresponds to the fields and functions in `Calculations`, q.v. for the
/// definitions.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkSource {
    Active,
    ActivePlusIoWait,
    ActiveMinusNice,
    User,
    Nice,
    System,
    Irq,
    SoftIrq,
    Steal,
    Guest,
    GuestNice,
    Idle,
    IoWait,
}

#[derive(Debug)]
pub struct InvalidWorkSource(String);

impl fmt::Display for InvalidWorkSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid value for worksource: '{}'", self.0)
    }
}

impl ::std::str::FromStr for WorkSource {
    type Err = InvalidWorkSource;
    fn from_str(s: &str) -> StdResult<WorkSource, InvalidWorkSource> {
        match s {
            "active" => Ok(WorkSource::Active),
            "activeplusiowait" => Ok(WorkSource::ActivePlusIoWait),
            "activeminusnice" => Ok(WorkSource::ActiveMinusNice),
            "use" => Ok(WorkSource::User),
            "nice" => Ok(WorkSource::Nice),
            "system" => Ok(WorkSource::System),
            "irq" => Ok(WorkSource::Irq),
            "softirq" => Ok(WorkSource::SoftIrq),
            "steal" => Ok(WorkSource::Steal),
            "guest" => Ok(WorkSource::Guest),
            "idle" => Ok(WorkSource::Idle),
            "iowait" => Ok(WorkSource::IoWait),
            _ => Err(InvalidWorkSource(s.to_string())),
        }
    }
}

impl fmt::Display for WorkSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::WorkSource::*;
        let s = match *self {
            Active => "active",
            ActivePlusIoWait => "active+iowait",
            ActiveMinusNice => "active-nice",
            User => "user",
            Nice => "nice",
            System => "system",
            Idle => "idle",
            IoWait => "iowait",
            Irq => "irq",
            SoftIrq => "softirq",
            Steal => "steal",
            Guest => "guest",
            GuestNice => "guestnice",
        };
        f.write_str(s)
    }
}

/// The number of calculations that have occured on this computer in a time
/// period
#[derive(PartialEq, Debug, Clone)]
pub struct Calculations {
    /// Time spent in user mode.
    pub user: Jiffies,
    /// Time spent in user mode with low priority (nice).
    pub nice: Jiffies,
    /// Time spent in system mode.
    pub system: Jiffies,
    /// Time spent not doing anything in particular
    pub idle: Jiffies,
    /// Time forced to be idle waiting for I/O to complete.
    pub iowait: Jiffies,
    /// Time spent servicing hardware interrupts
    pub irq: Jiffies,
    /// Time spent servicing software interrupts
    pub softirq: Jiffies,
    /// Stolen time, which is the time spent in other operating systems when
    /// running in a virtualized environment
    pub steal: Jiffies,
    /// Time spent running a virtual CPU for a guest operating system. AKA time
    /// we lent out to virtual machines.
    pub guest: Jiffies,
    /// Time spent running a niced guest
    pub guest_nice: Option<Jiffies>,
}

impl Calculations {
    /// Read /proc/stat and return its contents as a string
    fn read_procstat() -> Result<String> {
        let mut fh = try!(File::open("/proc/stat"));
        let mut contents = String::new();
        try!(fh.read_to_string(&mut contents));
        Ok(contents)
    }

    /// Build a new `Calculations` for *total* CPU jiffies from the /proc/stat
    /// pseudofile. See `load_per_cpu` for per-cpu metrics.
    pub fn load() -> Result<Calculations> {
        let contents = try!(Self::read_procstat());
        Self::from_str(&contents)
    }

    /// Build a list of per-cpu metrics from the /proc/stat file
    ///
    /// This does not include the `total` line, use `load()` for that.
    pub fn load_per_cpu() -> Result<Vec<Calculations>> {
        let contents = Self::read_procstat().unwrap();
        Calculations::per_cpu(&contents)
    }

    /// Create a Vec of Calculations for each individual cpu in a stat file
    fn per_cpu(contents: &str) -> Result<Vec<Calculations>> {
        contents
            .lines()
            .skip(1)
            .filter(|line| line.starts_with("cpu"))
            .map(Calculations::from_line)
            .collect::<StdResult<Vec<_>, _>>()
    }

    /// Convert a single line from /proc/stat
    fn from_line(line: &str) -> Result<Calculations> {
        assert!(line.starts_with("cpu"));

        let usages = try!(
            line.split(' ')
                .skip(1)
                .filter(|part| part.len() > 0)
                .map(|part| part.parse())
                .collect::<StdResult<Vec<u64>, _>>()
        );
        Ok(Calculations {
            user: Jiffies::new(usages[0]),
            nice: Jiffies::new(usages[1]),
            system: Jiffies::new(usages[2]),
            idle: Jiffies::new(usages[3]),
            iowait: Jiffies::new(usages[4]),
            irq: Jiffies::new(usages[5]),
            softirq: Jiffies::new(usages[6]),
            steal: Jiffies::new(usages[7]),
            guest: Jiffies::new(usages[8]),
            guest_nice: usages.get(9).map(|v| Jiffies::new(*v)),
        })
    }

    /// Jiffies spent non-idle
    ///
    /// This includes all processes in user space, kernel space, and time
    /// stolen by other VMs.
    #[cfg_attr(rustfmt, rustfmt_skip)]
    pub fn active(&self) -> Jiffies {
        self.user + self.nice + // user processes
            self.system + self.irq + self.softirq + // kernel and interrupts
            self.steal  // other VMs stealing our precious time
    }

    /// Jiffies spent with nothing to do
    pub fn idle(&self) -> Jiffies {
        self.idle + self.iowait
    }

    /// Jiffies spent running child VMs
    ///
    /// This is included in `active()` by the nature of the way that
    /// /proc/stats reports things, so don't add this to that when totalling.
    #[allow(dead_code)] // this mostly exists as documentation of what `guest` means
    pub fn virt(&self) -> Jiffies {
        self.guest + self.guest_nice.unwrap_or_else(|| Jiffies::new(0))
    }

    /// All jiffies since the kernel started tracking
    pub fn total(&self) -> Jiffies {
        self.active() + self.idle()
    }

    /// Return how much cpu time the specific worksource took
    ///
    /// The number returned is between 0 and 100
    pub fn percent_util_since(&self, kind: &WorkSource, start: &Calculations) -> f64 {
        let (start_val, end_val) = match *kind {
            WorkSource::Active => (start.active(), self.active()),
            WorkSource::ActivePlusIoWait => {
                (start.active() + start.iowait, self.active() + self.iowait)
            }
            WorkSource::ActiveMinusNice => (start.active() - start.nice, self.active() - self.nice),
            WorkSource::User => (start.user, self.user),
            WorkSource::IoWait => (start.iowait, self.iowait),
            WorkSource::Nice => (start.nice, self.nice),
            WorkSource::System => (start.system, self.system),
            WorkSource::Idle => (start.idle, self.idle),
            WorkSource::Irq => (start.irq, self.irq),
            WorkSource::SoftIrq => (start.softirq, self.softirq),
            WorkSource::Steal => (start.steal, self.steal),
            WorkSource::Guest => (start.guest, self.guest),
            WorkSource::GuestNice => (
                start.guest_nice.unwrap_or_else(|| Jiffies::new(0)),
                self.guest_nice.unwrap_or_else(|| Jiffies::new(0)),
            ),
        };
        assert!(self.total() >= start.total());
        let total = (end_val - start_val) / (self.total() - start.total());
        total * 100f64
    }
}

impl FromStr for Calculations {
    type Err = ProcFsError;
    /// Parse the entire /proc/stat file into a single `Calculations` object
    /// for total CPU
    fn from_str(contents: &str) -> Result<Calculations> {
        let mut calcs = try!(
            contents
                .lines()
                .take(1)
                .map(Self::from_line)
                .collect::<StdResult<Vec<_>, _>>()
        );
        Ok(calcs.remove(0))
    }
}

impl Sub for Calculations {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Calculations {
            user: self.user - rhs.user,
            nice: self.nice - rhs.nice,
            system: self.system - rhs.system,
            idle: self.idle - rhs.idle,
            iowait: self.iowait - rhs.iowait,
            irq: self.irq - rhs.irq,
            softirq: self.softirq - rhs.softirq,
            steal: self.steal - rhs.steal,
            guest: self.guest - rhs.guest,
            guest_nice: match (self.guest_nice, rhs.guest_nice) {
                (Some(lhs), Some(rhs)) => Some(lhs - rhs),
                _ => None,
            },
        }
    }
}

impl<'a> Sub for &'a Calculations {
    type Output = Calculations;
    fn sub(self, rhs: Self) -> Calculations {
        Calculations {
            user: self.user - rhs.user,
            nice: self.nice - rhs.nice,
            system: self.system - rhs.system,
            idle: self.idle - rhs.idle,
            iowait: self.iowait - rhs.iowait,
            irq: self.irq - rhs.irq,
            softirq: self.softirq - rhs.softirq,
            steal: self.steal - rhs.steal,
            guest: self.guest - rhs.guest,
            guest_nice: match (self.guest_nice, rhs.guest_nice) {
                (Some(lhs), Some(rhs)) => Some(lhs - rhs),
                _ => None,
            },
        }
    }
}

impl fmt::Display for Calculations {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let total = self.total();
        let p = |v: Jiffies| 100 * v / total;
        write!(
            f,
            "user={:.1} system={:.1} nice={:.1} irq={:.1} softirq={:.1} | idle={:.1} \
             iowait={:.1} | steal={:.1} guest={:.1} guest_nice={}",
            p(self.user),
            p(self.system),
            p(self.nice),
            p(self.irq),
            p(self.softirq),
            p(self.idle),
            p(self.iowait),
            p(self.steal),
            p(self.guest),
            self.guest_nice
                .map_or("unknown".into(), |v| format!("{}", v),)
        )
    }
}

// ////////////////////////////////////////////////////////////////////////////
// Memory

/// A struct that represents overall memory usage on the system.
///
/// All values are in KB.
#[derive(PartialEq, Eq, Debug)]
pub struct MemInfo {
    pub total: Option<usize>,
    pub available: Option<usize>,
    pub free: Option<usize>,
    pub cached: Option<usize>,
}

impl MemInfo {
    /// Read the data from /proc/meminfo into a `MemInfo`
    pub fn load() -> MemInfo {
        let contents = match File::open("/proc/meminfo") {
            Ok(ref mut content) => {
                let mut s = String::new();
                let _ = content.read_to_string(&mut s);
                s
            }
            Err(e) => panic!("Unable to open /proc/meminfo: {:?}", e),
        };

        MemInfo::from_str(&contents).unwrap()
    }
    /// Try to figure out how much memory is being used
    ///
    /// Since linux kernel 3.16 this just performs
    ///
    /// ```text
    ///     available / total * 100
    /// ```
    ///
    /// Before that we approximate available as Free + Cached, even though that
    /// is [almost certain to be incorrect][].
    ///
    /// [almost certain to be incorrect]: https://github.com/torvalds/linux/commit/34e431b0ae398fc54ea69ff85ec700722c9da773
    pub fn percent_free(&self) -> Result<f64> {
        match *self {
            MemInfo {
                total: Some(t),
                available: Some(a),
                ..
            } => Ok(a as f64 / t as f64 * 100.0),
            MemInfo {
                total: Some(t),
                free: Some(f),
                cached: Some(c),
                ..
            } => Ok((f + c) as f64 / t as f64 * 100.0),
            _ => Err(ProcFsError::InsufficientData(format!(
                "/proc/meminfo is missing one of \
                 total, available, free, or cached: \
                 {:?}",
                self
            ))),
        }
    }

    /// The inverse of `MemInfo::percent_free`
    pub fn percent_used(&self) -> Result<f64> {
        let free = try!(self.percent_free());
        Ok(100f64 - free)
    }
}

impl FromStr for MemInfo {
    type Err = ProcFsError;

    /// Convert the contents of a string like /proc/meminfo into a MemInfo
    /// object
    fn from_str(meminfo: &str) -> Result<Self> {
        enum Currently {
            Total,
            Free,
            Available,
            Cached,
            Unknown,
            None,
        };

        let mut word = String::new();
        let mut info = MemInfo {
            total: None,
            free: None,
            available: None,
            cached: None,
        };
        let mut amount: usize;
        let mut currently = Currently::None;

        for chr in meminfo.chars() {
            match chr {
                c if 'A' <= c && c <= 'z' => {
                    word.push(chr);
                }
                ':' => {
                    match &word[..] {
                        "MemTotal" => currently = Currently::Total,
                        "MemAvailable" => currently = Currently::Available,
                        "MemFree" => currently = Currently::Free,
                        "Cached" => currently = Currently::Cached,
                        _ => currently = Currently::Unknown,
                    }
                    word.clear();
                }
                x if '0' <= x && x <= '9' => {
                    word.push(chr);
                }
                ' ' | '\n' => {
                    if word.is_empty() {
                        continue;
                    }
                    if word == "kB" {
                        word.clear();
                        continue;
                    }

                    amount = match word.parse() {
                        Ok(amount) => amount,
                        Err(e) => panic!(r#"Unable to parse number from "{}": {:?}"#, word, e),
                    };
                    word.clear();

                    match currently {
                        Currently::Total => info.total = Some(amount),
                        Currently::Free => info.free = Some(amount),
                        Currently::Available => info.available = Some(amount),
                        Currently::Cached => info.cached = Some(amount),
                        Currently::Unknown => {
                            // don't care
                        }
                        Currently::None => panic!(
                            "Unexpectedly parsed a number before figuring out where I am: \
                             {}",
                            amount
                        ),
                    }
                }
                _ => {
                    // Don't care about other chars
                }
            }
        }

        Ok(info)
    }
}

/// The load average of the system
///
/// Load average is number of jobs in the run queue (state R) or waiting for
/// disk I/O (state D) averaged over 1, 5, and 15 minutes.
#[derive(PartialEq, PartialOrd, Debug, Deserialize)]
pub struct LoadAvg {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

impl LoadAvg {
    /// Load from the /proc/loadavg file
    pub fn load() -> Result<LoadAvg> {
        let mut fh = try!(File::open("/proc/loadavg"));
        let mut contents = String::new();
        try!(fh.read_to_string(&mut contents));
        Self::from_str(&contents)
    }
}

impl Div<usize> for LoadAvg {
    type Output = LoadAvg;

    /// Divide by an integer. Useful to divide by the number of CPUs
    fn div(self, rhs: usize) -> LoadAvg {
        LoadAvg {
            one: self.one / rhs as f64,
            five: self.five / rhs as f64,
            fifteen: self.fifteen / rhs as f64,
        }
    }
}

impl FromStr for LoadAvg {
    type Err = ProcFsError;

    fn from_str(contents: &str) -> Result<LoadAvg> {
        let pat = Regex::new(r"[ ,]").unwrap();
        let fields = try!(
            pat.split(contents)
                .take(3)
                .map(|load| load.parse())
                .collect::<StdResult<Vec<f64>, _>>()
        );
        Ok(LoadAvg {
            one: fields[0],
            five: fields[1],
            fifteen: fields[2],
        })
    }
}

impl fmt::Display for LoadAvg {
    fn fmt(&self, f: &mut fmt::Formatter) -> StdResult<(), fmt::Error> {
        try!(write!(
            f,
            "{:.1} {:.1} {:.1}",
            self.one, self.five, self.fifteen
        ));
        Ok(())
    }
}

// ////////////////////////////////////////////////////////////////////////////
// Disks

#[derive(Debug, PartialEq, Eq)]
pub struct Mount {
    pub spec: String,
    pub file: String,
    pub vfstype: String,
    pub mntops: Vec<String>,
    pub freq: Option<u32>,
    pub passno: Option<u32>,
}

fn next(parts: &mut Split<char>) -> Result<String> {
    if let Some(part) = parts.next() {
        Ok(part.to_owned())
    } else {
        Err(ProcFsError::InsufficientData(
            "Missing part from mount".to_owned(),
        ))
    }
}

fn mount_from_line(line: &str) -> Result<Mount> {
    use self::ProcFsError::InsufficientData;
    let mut parts = line.split(' ');
    Ok(Mount {
        spec: try!(next(&mut parts)),
        file: try!(next(&mut parts)),
        vfstype: try!(next(&mut parts)),
        mntops: try!(parts.next().map_or(
            Err(InsufficientData("Missing mnt ops from mount".to_owned(),)),
            Ok,
        )).split(',')
            .map(|part| part.to_owned())
            .collect::<Vec<_>>(),
        freq: parts.next().map(|v| v.parse().unwrap()),
        passno: parts.next().map(|v| v.parse().unwrap()),
    })
}

impl Mount {
    fn read_mounts() -> Result<String> {
        let mut fh = try!(File::open("/proc/mounts"));
        let mut contents = String::new();
        try!(fh.read_to_string(&mut contents));
        Ok(contents)
    }

    fn parse_str(mounts: &str) -> Result<Vec<Mount>> {
        mounts
            .lines()
            .map(mount_from_line)
            .collect::<Result<Vec<_>>>()
    }

    pub fn load_all() -> Result<Vec<Mount>> {
        let mounts = try!(Mount::read_mounts());
        Mount::parse_str(&mounts)
    }
}

// ////////////////////////////////////////////////////////////////////////////
// Testing

#[cfg(test)]
mod unit {
    use super::*;
    use super::mount_from_line;
    use std::str::FromStr;

    use linux::Jiffies;

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn can_parse_stat_for_system() {
        let c = Calculations::from_str(
"cpu  100 55 66 77 88 1 9 0 0 0
cpu0 415415 55 55572 37240261 7008 1 1803 0 0 0
intr 17749885 52 10 0 0 0 0 0 0 0 0 0 0 149 0 0 0 0 0 0 493792 10457659 2437665
ctxt 310
btime 143
");
        assert_eq!(c.unwrap(),
                   Calculations {
                       user: Jiffies::new(100),
                       nice: Jiffies::new(55),
                       system: Jiffies::new(66),
                       idle: Jiffies::new(77),
                       iowait: Jiffies::new(88),
                       irq: Jiffies::new(1),
                       softirq: Jiffies::new(9),
                       steal: Jiffies::new(0),
                       guest: Jiffies::new(0),
                       guest_nice: Some(Jiffies::new(0)),
                   });
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn can_parse_multiple_cpus() {
        let c = Calculations::per_cpu(
"cpu  100 55 66 77 88 1 9 0 0 0
cpu0 101 99 99 99 99 99 99 0 0 0
cpu1 102 98 98 98 98 98 98 0 0 0
cpu1 103 97 97 97 97 97 97 0 0 0
intr 17749885 52 10 0 0 0 0 0 0 0 0 0 0 149 0 0 0 0 0 0 493792 10457659 2437665
ctxt 310
btime 143
");
        assert_eq!(c.unwrap(),
                   vec![Calculations {
                            user: Jiffies::new(101),
                            nice: Jiffies::new(99),
                            system: Jiffies::new(99),
                            idle: Jiffies::new(99),
                            iowait: Jiffies::new(99),
                            irq: Jiffies::new(99),
                            softirq: Jiffies::new(99),
                            steal: Jiffies::new(0),
                            guest: Jiffies::new(0),
                            guest_nice: Some(Jiffies::new(0)),
                        },
                        Calculations {
                            user: Jiffies::new(102),
                            nice: Jiffies::new(98),
                            system: Jiffies::new(98),
                            idle: Jiffies::new(98),
                            iowait: Jiffies::new(98),
                            irq: Jiffies::new(98),
                            softirq: Jiffies::new(98),
                            steal: Jiffies::new(0),
                            guest: Jiffies::new(0),
                            guest_nice: Some(Jiffies::new(0)),
                        },
                        Calculations {
                            user: Jiffies::new(103),
                            nice: Jiffies::new(97),
                            system: Jiffies::new(97),
                            idle: Jiffies::new(97),
                            iowait: Jiffies::new(97),
                            irq: Jiffies::new(97),
                            softirq: Jiffies::new(97),
                            steal: Jiffies::new(0),
                            guest: Jiffies::new(0),
                            guest_nice: Some(Jiffies::new(0)),
                        }]);
    }

    #[test]
    fn can_parse_stat_for_process() {
        let stat = "1 (init) S 0 1 1 0 -1 4219136 40326 5752369 36 3370 16 41 25846 8061 20 0 1 0 \
                    5 34381824 610 18446744073709551615 1 1 0 0 0 0 0 4096 536962595 \
                    18446744073709551615 0 0 17 0 0 0 7 0 0 0 0 0 0 0 0 0 0";
        stat.parse::<pid::Stat>().unwrap();
    }

    #[test]
    fn parse_meminfo() {
        assert_eq!(
            MemInfo::from_str(concat!(
                "Useless: 898\n",
                "MemTotal: 500\n",
                "MemAvailable: 20\n",
                "MemFree: 280\n",
                "Meaningless: 777\n",
                "Cached: 200\n"
            )).unwrap(),
            MemInfo {
                total: Some(500),
                available: Some(20),
                free: Some(280),
                cached: Some(200),
            }
        )
    }

    #[test]
    fn meminfo_percent_free() {
        let mem = MemInfo {
            total: Some(100),
            available: Some(30),
            free: None,
            cached: None,
        };
        assert_eq!(mem.percent_free().unwrap(), 30.0);

        let mem = MemInfo {
            total: Some(100),
            available: None,
            free: Some(25),
            cached: Some(20),
        };
        assert_eq!(mem.percent_free().unwrap(), 45.0);
    }

    #[test]
    fn loadavg_can_parse_space_str() {
        let avg = LoadAvg::from_str("0.1 1.5 21 5/23 938").unwrap();
        assert_eq!(
            avg,
            LoadAvg {
                one: 0.1,
                five: 1.5,
                fifteen: 21.0,
            }
        );
    }

    #[test]
    fn loadavg_can_parse_comma_str() {
        let avg = LoadAvg::from_str("0.1,1.5,21").unwrap();
        assert_eq!(
            avg,
            LoadAvg {
                one: 0.1,
                five: 1.5,
                fifteen: 21.0,
            }
        );
    }

    #[test]
    fn loadavg_display() {
        let string = format!(
            "{}",
            LoadAvg {
                one: 0.888,
                five: 1.0,
                fifteen: 0.1,
            }
        );
        // ooh, rounding
        assert_eq!(&string, "0.9 1.0 0.1");
    }

    #[test]
    fn mount_from_line_works() {
        let line = "none /data/docker/aufs/mnt/b6e1b aufs \
                    rw,relatime,si=5c1d022653bfa828,dio,dirperm1 0 0";
        let mount = mount_from_line(&line).unwrap();

        fn s(st: &str) -> String {
            st.to_owned()
        }
        assert_eq!(
            mount,
            Mount {
                spec: s("none"),
                file: s("/data/docker/aufs/mnt/b6e1b"),
                vfstype: s("aufs"),
                mntops: vec![
                    s("rw"),
                    s("relatime"),
                    s("si=5c1d022653bfa828"),
                    s("dio"),
                    s("dirperm1"),
                ],
                freq: Some(0),
                passno: Some(0),
            }
        )
    }

    #[test]
    fn mount_all_works() {
        let mount_config = "none /data/docker/aufs/mnt/b6e1b aufs \
                            rw,relatime,si=5c1d022653bfa828,dio,dirperm1 0 0\nsome / ext4 \
                            rw,relatime 0\n";
        let mounts = Mount::parse_str(&mount_config).unwrap();

        fn s(st: &str) -> String {
            st.to_owned()
        }
        assert_eq!(
            mounts,
            vec![
                Mount {
                    spec: s("none"),
                    file: s("/data/docker/aufs/mnt/b6e1b"),
                    vfstype: s("aufs"),
                    mntops: vec![
                        s("rw"),
                        s("relatime"),
                        s("si=5c1d022653bfa828"),
                        s("dio"),
                        s("dirperm1"),
                    ],
                    freq: Some(0),
                    passno: Some(0),
                },
                Mount {
                    spec: s("some"),
                    file: s("/"),
                    vfstype: s("ext4"),
                    mntops: vec![s("rw"), s("relatime")],
                    freq: Some(0),
                    passno: None,
                },
            ]
        )
    }

    #[test]
    fn worksource_can_parse_from_str() {
        for src in [
            "active",
            "activeplusiowait",
            "activeminusnice",
            "use",
            "nice",
            "system",
            "irq",
            "softirq",
            "steal",
            "guest",
            "idle",
            "iowait",
        ].iter()
        {
            if let Err(e) = src.parse::<WorkSource>() {
                panic!("Error parsing worksource for '{}': {}", src, e);
            }
        }
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod integration {
    use super::{LoadAvg, MemInfo, Mount, RunningProcs};

    #[test]
    fn can_read_all_procs() {
        let procs = RunningProcs::currently_running().unwrap();
        assert!(procs.0.len() > 0);
    }

    #[test]
    fn meminfo_can_load() {
        let info = MemInfo::load();
        assert_eq!(
            info.percent_free().unwrap() + info.percent_used().unwrap(),
            100.0
        )
    }

    #[test]
    fn loadavg_can_load() {
        LoadAvg::load().unwrap();
    }

    #[test]
    fn mount_can_load() {
        Mount::load_all().unwrap();
    }
}

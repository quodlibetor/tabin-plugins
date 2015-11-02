//! Standard APIs in Linux

use std::fmt;
use std::ops;
use std::time::Duration;

use libc::consts::os::sysconf::{_SC_CLK_TCK, _SC_PAGESIZE};
use libc::funcs::posix88::unistd::sysconf;

lazy_static!(
    pub static ref USER_HZ: u64 = unsafe { sysconf(_SC_CLK_TCK) } as u64;
    pub static ref PAGESIZE: u64 = unsafe { sysconf(_SC_PAGESIZE) } as u64;
);


pub fn pages_to_human_size(pages: u64) -> String {
    let bytes = pages * (*PAGESIZE);
    bytes_to_human_size(bytes)
}

fn bytes_to_human_size(bytes: u64) -> String {
    let mut bytes = bytes as f64;
    let sizes = ["B", "K", "M", "G", "T"];
    let mut reductions = 0;
    while reductions < sizes.len() - 1 {
        if bytes > 1000.0 {
            bytes = bytes / 1024.0;
            reductions += 1;
        } else {
            break;
        }
    }
    format!("{:>5.1}{}", bytes, sizes[reductions])
}

#[test]
fn pages_to_human_size_produces_shortest() {
    let reprs = [(999,                    "999.0B"),
                 (9_999,                  "  9.8K"),
                 (9_999_999,              "  9.5M"),
                 (35_999_999,             " 34.3M"),
                 (9_999_999_999,          "  9.3G"),
                 (9_999_999_999_999,      "  9.1T"),
                 (90_999_999_999_999_999, "82764.0T")];

    reprs.iter().map(|&(raw, repr): &(u64, &str)|
                     assert_eq!(bytes_to_human_size(raw), repr)).collect::<Vec<_>>();
}

/// A value that is in USER_HZ units
///
/// This generally represents some time period of CPU usage
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UserHz(u64);

impl UserHz {
    pub fn new(val: u64) -> UserHz {
        UserHz(val)
    }

    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.0 * 1000)
    }
}

impl fmt::Display for UserHz {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "UserHz({})", self.0)
    }
}

impl ops::Add for UserHz {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        UserHz(self.0 + rhs.0)
    }
}

impl ops::Sub for UserHz {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        UserHz(self.0 - rhs.0)
    }
}

/// A count of Jiffies
///
/// This represents some value read from the /proc fs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Jiffies(u64);

impl Jiffies {
    pub fn new(val: u64) -> Jiffies {
        Jiffies(val)
    }

    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.0 * 1000 / *USER_HZ)
    }
}

impl From<u64> for Jiffies {
    fn from(u: u64) -> Jiffies {
        Jiffies::new(u)
    }
}

impl From<i64> for Jiffies {
    fn from(u: i64) -> Jiffies {
        Jiffies::new(u as u64)
    }
}

impl fmt::Display for Jiffies {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Jiffies({})", self.0)
    }
}

impl ops::Add for Jiffies {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Jiffies(self.0 + rhs.0)
    }
}

impl ops::Sub for Jiffies {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Jiffies(self.0 - rhs.0)
    }
}

impl ops::Div for Jiffies {
    type Output = f64;
    fn div(self, rhs: Self) -> f64 {
        self.0 as f64 / rhs.0 as f64
    }
}

impl ops::Div<Jiffies> for u64 {
    type Output = f64;
    fn div(self, rhs: Jiffies) -> Self::Output {
        self as f64 / rhs.0 as f64
    }
}

impl ops::Mul<Jiffies> for u64 {
    type Output = Self;
    fn mul(self, rhs: Jiffies) -> Self::Output {
        self * rhs.0
    }
}

pub trait Ratio<Rhs = Self> {
    fn ratio(&self, rhs: &Rhs) -> f64;
}

impl Ratio for Duration {
    fn ratio(&self, rhs: &Self) -> f64 {
        let my_nanos = self.as_secs() *  1_000_000_000 + self.subsec_nanos() as u64;
        let rhs_nanos = rhs.as_secs() *  1_000_000_000 + rhs.subsec_nanos() as u64;
        my_nanos as f64 / rhs_nanos as f64
    }
}

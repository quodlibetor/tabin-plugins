//! Interact with the `/sys/fs` file system

use std::fs::File;
use std::io::{self, Read};

fn read_file(path: &str) -> Result<String, io::Error> {
    let mut fh = try!(File::open(path));
    let mut contents = String::new();
    try!(fh.read_to_string(&mut contents));
    Ok(contents)
}

pub mod fs {
    pub mod cgroup {
        pub mod cpuacct {
            //! The cpuacct directory for describing cgroups
            //!
            //! The most interesting file in here is cpuacct.stat which shows
            //! the total cpu usage for all processes in this cgroup
            use std::io;

            use linux::UserHz;
            use sys::read_file;

            #[derive(Debug)]
            pub struct Stat {
                pub user: UserHz,
                pub system: UserHz,
            }

            impl Stat {
                pub fn load() -> Result<Stat, io::Error> {
                    let contents = try!(read_file("/sys/fs/cgroup/cpuacct/cpuacct.stat"));
                    let mut lines = contents.lines();
                    let user = lines.next().unwrap().split(" ").nth(1).unwrap();
                    let sys = lines.next().unwrap().split(" ").nth(1).unwrap();

                    Ok(Stat {
                        user: UserHz::new(user.parse().unwrap()),
                        system: UserHz::new(sys.parse().unwrap()),
                    })
                }

                pub fn total(&self) -> UserHz {
                    self.user + self.system
                }
            }
        }

        pub mod memory {
            //! The memory directory for describing cgroups
            //!
            //! See the
            //! [memory.txt](https://www.kernel.org/doc/Documentation/cgroups/memory.txt)
            //! file.

            use std::collections::{HashSet, HashMap};
            use std::io;

            use sys::read_file;

            /// The memory limit for this cgroup
            ///
            /// If it's not set, it seems to be u64::max
            pub fn limit_in_bytes() -> Result<usize, io::Error> {
                let contents = try!(read_file("/sys/fs/cgroup/memory/memory.limit_in_bytes"));
                let bytes = contents.trim().parse().unwrap();
                Ok(bytes)
            }

            /// Some fields from the memory.stat file
            ///
            /// All values are in bytes, contrary to the same file from procfs
            /// which reports everything in pages
            #[derive(Debug)]
            pub struct Stat {
                /// Memory used, including the filesystem page cache. This
                /// number will never decrease unless memory presure gets
                /// applied from outside the cgroup.
                pub cache: usize,
                /// Actual memory being used by the cgroup
                pub rss: usize,
                /// Only hugetables. This is included in rss
                pub rss_huge: usize,
                /// Number of bytes that have been swapped out to disk
                pub swap: usize,
            }

            impl Stat {
                /// Read information from the filesystem and create a new `Stat`
                pub fn load() -> Result<Stat, io::Error> {
                    let contents = try!(read_file("/sys/fs/cgroup/memory/memory.stat"));
                    let mut fields: HashMap<String, usize> = HashMap::new();
                    let needed: HashSet<_> = ["cache", "rss", "rss_huge", "swap"]
                                                 .iter()
                                                 .cloned()
                                                 .collect();
                    let mut found = 0;
                    for line in contents.lines() {
                        let mut parts = line.split(" ");
                        let field = parts.next().unwrap();
                        let val = parts.next().unwrap();
                        if needed.contains(&field) {
                            fields.insert(field.to_owned(), val.parse().unwrap());
                            found += 1;
                            if found >= 4 {
                                break;
                            }
                        }
                    }

                    Ok(Stat {
                        cache: *fields.get("cache").unwrap(),
                        rss: *fields.get("rss").unwrap(),
                        rss_huge: *fields.get("rss_huge").unwrap(),
                        swap: *fields.get("swap").unwrap(),
                    })
                }
            }
        }
    }
}

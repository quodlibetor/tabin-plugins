//! Interact with the `/sys/fs` file system

pub mod fs {
    pub mod cgroup {
        pub mod cpuacct {
            //! The cpuacct directory for describing cgroups
            //!
            //! The most interesting file in here is cpuacct.stat which shows
            //! the total cpu usage for all processes in this cgroup
            use std::fs::File;
            use std::io::{self, Read};

            use linux::UserHz;

            #[derive(Debug)]
            pub struct Stat {
                pub user: UserHz,
                pub system: UserHz
            }

            impl Stat {
                fn read() -> Result<String, io::Error> {
                    let mut fh = try!(File::open("/sys/fs/cgroup/cpuacct/cpuacct.stat"));
                    let mut contents = String::new();
                    try!(fh.read_to_string(&mut contents));
                    Ok(contents)
                }

                pub fn load() -> Result<Stat, io::Error> {
                    let contents = try!(Self::read());
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
    }
}

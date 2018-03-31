//! Check running processes

#![allow(unused_variables, dead_code)]

extern crate nix;
extern crate regex;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

extern crate tabin_plugins;

use nix::unistd::{getpid, getppid, Pid};
use regex::Regex;
use structopt::StructOpt;

use tabin_plugins::Status;
use tabin_plugins::procfs::{LoadProcsError, ProcFsError, ProcMap, RunningProcs};
use tabin_plugins::procfs::pid::{Process, State};

/// Check that an expected number of processes are running.
#[derive(StructOpt, Debug, Deserialize)]
#[structopt(name = "check-procs (part of tabin-plugins)",
            raw(setting = "structopt::clap::AppSettings::ColoredHelp"),
            after_help = "Examples:

    Ensure at least two nginx processes are running:

        check-procs --crit-under 2 nginx

    Ensure there are not more than 30 zombie proccesses on the system:

        check-procs --crit-over 30 --state zombie

    Ensure that there are not more than 5 java processes running MyMainClass
    that are in the zombie *or* waiting states:

        check-procs --crit-over 5 --state zombie --state waiting 'java.*MyMainClass'

    Ensure that there are at least three (running or waiting) (cassandra or
    postgres) processes:

        check-procs --crit-under 3 --state running --state waiting 'cassandra|postgres'")]
struct Args {
    #[structopt(help = "Regex that command and its arguments must match")]
    pattern: Option<String>,
    #[structopt(long = "crit-under", name = "N",
                help = "Error if there are fewer than <N> procs matching <pattern>")]
    crit_under: Option<usize>,
    #[structopt(long = "crit-over", name = "M",
                help = "Error if there are more than <M> procs matching <pattern>")]
    crit_over: Option<usize>,

    #[structopt(long = "state",
                help = "Filter to only processes in these states. \
                        If passed multiple times, processes matching any state are included.\n\
                        Choices: running sleeping uninterruptible-sleep waiting stopped zombie")]
    states: Vec<State>,

    #[structopt(long = "allow-unparseable-procs",
                help = "In combination with --crit-over M this will not alert if any \
                        processes cannot be parsed")]
    allow_unparseable_procs: bool,
}

fn parse_args() -> (Option<Regex>, Args) {
    let args = Args::from_args();
    let mut re = None;
    if let &Some(ref pattern) = &args.pattern {
        re = Some(Regex::new(&pattern).unwrap_or_else(|e| {
            println!("ERROR: invalid process pattern: {}", e);
            Status::Critical.exit();
        }));
    } else if args.states.is_empty() {
        println!("At least one of a pattern or some states are required for this to do anything");
        Status::Critical.exit();
    }
    if let (None, None) = (args.crit_under, args.crit_over) {
        println!("At least one of --crit-under or --crit-over must be provided");
        Status::Critical.exit();
    }
    (re, args)
}

fn main() {
    let (re, args) = parse_args();
    let should_die = if let Some(_) = args.crit_over {
        !args.allow_unparseable_procs
    } else {
        false
    };
    let procs = load_procs(should_die);

    let matches = filter_procs(re, &args.states, &procs.0);

    let mut status = Status::Ok;
    if let Some(crit_over) = args.crit_over {
        if matches.len() > crit_over {
            status = Status::Critical;
            println!(
                "CRITICAL: there are {} process that match {:?} (greater than {})",
                matches.len(),
                args.pattern,
                crit_over
            );
        }
    };
    if let Some(crit_under) = args.crit_under {
        if matches.len() < crit_under {
            status = Status::Critical;
            println!(
                "CRITICAL: there are {} process that match {:?} (less than {})",
                matches.len(),
                args.pattern,
                crit_under
            );
        }
    }

    if status == Status::Ok {
        match (args.crit_over, args.crit_under) {
            (Some(o), Some(u)) => println!(
                "OKAY: There are {} matching procs (between {} and {})",
                matches.len(),
                o,
                u
            ),
            (Some(o), None) => println!(
                "OKAY: There are {} matching procs (less than {})",
                matches.len(),
                o
            ),
            (None, Some(u)) => println!(
                "OKAY: There are {} matching procs (greater than {})",
                matches.len(),
                u
            ),
            (None, None) => unreachable!(),
        }
    }
    if matches.len() > 0 {
        println!("INFO: Matching processes:");
        for process in matches.iter().take(20) {
            println!("[{:>5}] {}", process.0, process.1.useful_cmdline());
        }
        if matches.len() > 20 {
            println!("And {} more...", matches.len() - 20)
        }
    }
    status.exit();
}

/// Load currently running procs, and die if there is a surprising error
///
/// Normally if this can load *any* processes it returns what it can find, and
/// prints errors for procs that can't be parsed. But if `die_on_any_errors` is
/// true it dies if it cannot parse a *single* process.
fn load_procs(die_on_any_errors: bool) -> RunningProcs {
    match RunningProcs::currently_running() {
        Ok(procs) => procs,
        Err(ProcFsError::LoadProcsError(LoadProcsError { procs, errors })) => {
            let mut saw_real_error = false;
            for err in errors {
                match err {
                    // If the process no longer exists, that's fine
                    ProcFsError::Io(_) => {}
                    err => {
                        saw_real_error = true;
                        println!("WARN: Unexpected error loading some processes: {}", err);
                    }
                }
            }
            if die_on_any_errors && saw_real_error {
                Status::Critical.exit();
            }
            procs
        }
        Err(err) => {
            println!("ERROR: unable to load processes: {}", err);
            Status::Critical.exit();
        }
    }
}

/// Filter to processes that match the condition
///
/// The condition is:
///
/// * Does not match me or my parent process
/// * Does match the regex, if it is present
/// * Does match *any* of the states
fn filter_procs<'m>(
    re: Option<Regex>,
    states: &[State],
    procs: &'m ProcMap,
) -> Vec<(&'m Pid, &'m Process)> {
    let me = getpid();
    let parent = getppid();
    let mut maybe: Box<Iterator<Item = (&Pid, &Process)>> = Box::new(
        procs
            .iter()
            .filter(|&(ref pid, ref process)| **pid != me && **pid != parent),
    );
    if !states.is_empty() {
        maybe =
            Box::new(maybe.filter(|&(ref pid, ref process)| states.contains(&process.stat.state)));
    }
    if let Some(re) = re {
        return Box::new(
            maybe.filter(|&(ref pid, ref process)| re.is_match(&process.useful_cmdline())),
        ).collect();
    } else {
        return maybe.collect();
    }
}

#[cfg(test)]
mod unit {
    use structopt::StructOpt;

    use tabin_plugins::procfs::pid::Process;

    use super::*;

    #[test]
    fn validate_argparse() {
        let args = Args::from_iter(["c-p", "some.*proc", "--crit-under=1"].into_iter());
        assert_eq!(args.crit_under, Some(1));
    }

    #[test]
    fn validate_parse_zombies() {
        let args = Args::from_iter(["c-p", "some.*proc", "--state=zombie"].into_iter());
        assert_eq!(args.states, [State::Zombie]);
        let args =
            Args::from_iter(["c-p", "some.*proc", "--state=zombie", "--state", "S"].into_iter());
        assert_eq!(args.states, [State::Zombie, State::Sleeping]);
    }


    #[test]
    fn filter_procs_handles_patterns() {
        let mut procs = vec![Process::default(); 5];
        procs[2].cmdline.raw.append(&mut vec!["hello".into(), "jar".into()]);
        let proc_map = vec_to_procmap(procs);
        let filtered = filter_procs(regex("llo.*ar"), &[], &proc_map);
        assert_eq!(filtered.len(), 1);
    }


    #[test]
    fn filter_procs_handles_single_state() {
        let mut procs = vec![Process::default(); 5];
        procs[2].stat.state = State::Zombie;
        let proc_map = vec_to_procmap(procs);
        let filtered = filter_procs(None, &[State::Zombie], &proc_map);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_procs_handles_multiple_states() {
        let mut procs = vec![Process::default(); 5];
        procs[2].stat.state = State::Zombie;
        procs[3].stat.state = State::Stopped;
        let proc_map = vec_to_procmap(procs);

        let filtered = filter_procs(None, &[State::Zombie], &proc_map);
        assert_eq!(filtered.len(), 1);

        let filtered = filter_procs(None, &[State::Zombie, State::Stopped], &proc_map);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_procs_handles_multiple_states_and_regex() {
        let mut procs = vec![Process::default(); 5];
        procs[2].stat.state = State::Zombie;
        procs[2].cmdline.raw.push("example".into());

        procs[3].stat.state = State::Stopped;
        let proc_map = vec_to_procmap(procs);

        let filtered = filter_procs(regex("exa"), &[], &proc_map);
        assert_eq!(filtered.len(), 1);

        let filtered = filter_procs(regex("exa"), &[State::Zombie, State::Stopped], &proc_map);
        assert_eq!(filtered.len(), 1);

        let filtered = filter_procs(regex("exa"), &[State::Stopped], &proc_map);
        assert_eq!(filtered.len(), 0);
    }

    fn regex(re: &str) -> Option<Regex> {
        Some(Regex::new(re).unwrap())
    }

    fn vec_to_procmap(procs: Vec<Process>) -> ProcMap {
        procs
            .into_iter()
            .enumerate()
            .map(|(i, process)| (Pid::from_raw(i as i32), process))
            .collect()
    }
}

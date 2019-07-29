//! Check running processes

use std::collections::HashSet;
use std::str::FromStr;

use log::LevelFilter::{Debug, Trace, Warn};
use log::{debug, trace};
use nix::sys::signal::{kill, Signal as NixSignal};
use nix::unistd::{getpid, getppid, Pid};
use regex::Regex;
use structopt::StructOpt;

use tabin_plugins::procfs::pid::{Process, State};
use tabin_plugins::procfs::{LoadProcsError, ProcFsError, ProcMap, RunningProcs};
use tabin_plugins::Status;

const LOG_VAR: &str = "TABIN_LOG";

/// Check that an expected number of processes are running.
///
/// Optionally, kill unwanted processes.
#[derive(StructOpt, Debug)]
#[structopt(
    name = "check-procs (part of tabin-plugins)",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp"),
    after_help = "Examples:

    Ensure at least two nginx processes are running:

        check-procs --crit-under 2 nginx

    Ensure there are not more than 30 zombie proccesses on the system:

        check-procs --crit-over 30 --state zombie

    Ensure that there are not more than 5 java processes running MyMainClass
    that are in the zombie *or* waiting states. Note that since there can be
    multiple states the regex must come before the `state` flag:

        check-procs 'java.*MyMainClass' --crit-over 5 --state zombie waiting

    Ensure that there are at least three (running or waiting) (cassandra or
    postgres) processes:

        check-procs --crit-under 3 --state=running --state=waiting 'cassandra|postgres'"
)]
struct Args {
    #[structopt(help = "Regex that command and its arguments must match")]
    pattern: Option<Regex>,
    #[structopt(
        long = "crit-under",
        name = "N",
        help = "Error if there are fewer than <N> procs matching <pattern>"
    )]
    crit_under: Option<usize>,
    #[structopt(
        long = "crit-over",
        name = "M",
        help = "Error if there are more than <M> procs matching <pattern>"
    )]
    crit_over: Option<usize>,

    #[structopt(
        long = "state",
        help = "Filter to only processes in these states. \
                If passed multiple times, processes matching any state are included.\n\
                Choices: running sleeping uninterruptible-sleep waiting stopped zombie"
    )]
    states: Vec<State>,

    #[structopt(
        long = "allow-unparseable-procs",
        help = "In combination with --crit-over M this will not alert if any \
                processes cannot be parsed"
    )]
    allow_unparseable_procs: bool,

    #[structopt(
        long = "kill-matching",
        name = "SIGNAL",
        help = "If *any* processes match, then kill them with the provided signal \
                which can be either an integer or a name like KILL or SIGTERM. \
                This option does not affect the exit status, all matches are always \
                killed, and if --crit-under/over are violated then then this will \
                still exit critical."
    )]
    kill_matching: Option<Signal>,

    #[structopt(
        long = "kill-parents-of-matching",
        name = "PARENT_SIGNAL",
        help = "If *any* processes match, then kill their parents with the provided \
                signal which can be either an integer or a name like KILL or SIGTERM. \
                This has the same exit status behavior as kill-matching."
    )]
    kill_matching_parents: Option<Signal>,

    /// print debug logs, use multiple times to make it more verbose
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,
}

/// Our own signal wrapper so that we can implement a forgiving FromStr for `nix::sys::Signal`
#[derive(Debug)]
struct Signal(NixSignal);

impl FromStr for Signal {
    type Err = String;
    fn from_str(s: &str) -> Result<Signal, String> {
        let sig: Result<i32, _> = s.parse();
        match sig {
            Ok(integer) => {
                Ok(Signal(NixSignal::from_c_int(integer).map_err(|_| {
                    format!("Not a valid signal integer: {}", s)
                })?))
            }
            Err(_) => Ok(Signal(match s {
                "SIGHUP" | "HUP" => NixSignal::SIGHUP,
                "SIGINT" | "INT" => NixSignal::SIGINT,
                "SIGQUIT" | "QUIT" => NixSignal::SIGQUIT,
                "SIGILL" | "ILL" => NixSignal::SIGILL,
                "SIGTRAP" | "TRAP" => NixSignal::SIGTRAP,
                "SIGABRT" | "ABRT" => NixSignal::SIGABRT,
                "SIGBUS" | "BUS" => NixSignal::SIGBUS,
                "SIGFPE" | "FPE" => NixSignal::SIGFPE,
                "SIGKILL" | "KILL" => NixSignal::SIGKILL,
                "SIGUSR1" | "USR1" => NixSignal::SIGUSR1,
                "SIGSEGV" | "SEGV" => NixSignal::SIGSEGV,
                "SIGUSR2" | "USR2" => NixSignal::SIGUSR2,
                "SIGPIPE" | "PIPE" => NixSignal::SIGPIPE,
                "SIGALRM" | "ALRM" => NixSignal::SIGALRM,
                "SIGTERM" | "TERM" => NixSignal::SIGTERM,
                _ => return Err(format!("Could not parse {:?} as an int or named signal", s)),
            })),
        }
    }
}

fn parse_args() -> Args {
    let args = Args::from_args();
    if args.pattern.is_none() && args.states.is_empty() {
        println!("At least one of a pattern or some states are required for this to do anything");
        Status::Critical.exit();
    }
    if let (None, None) = (args.crit_under, args.crit_over) {
        println!("At least one of --crit-under or --crit-over must be provided");
        Status::Critical.exit();
    }
    args
}

fn main() {
    let args = parse_args();
    env_logger::Builder::from_env(LOG_VAR)
        .filter_level(match args.verbose {
            0 => Warn,
            1 => Debug,
            _ => Trace,
        })
        .init();

    let should_die = if let Some(_) = args.crit_over {
        !args.allow_unparseable_procs
    } else {
        false
    };
    let procs = load_procs(should_die);
    let re_ = args.pattern.as_ref().map(|s| s.to_string());
    let re = || re_.as_ref().map(|s| &**s).unwrap_or("<ANYTHING>");

    let matches = filter_procs(&args.pattern, &args.states, &procs.0);

    let mut status = Status::Ok;
    if let Some(crit_over) = args.crit_over {
        if matches.len() > crit_over {
            status = Status::Critical;
        }
    };
    if let Some(crit_under) = args.crit_under {
        if matches.len() < crit_under {
            status = Status::Critical;
        }
    }

    print!("{}: there are {} procs ", status, matches.len());
    if args.pattern.is_some() {
        print!("that match '{}' ", re());
    }
    if !args.states.is_empty() {
        print!("with any state in {:?} ", args.states);
    }
    let prefix = if status == Status::Critical {
        "not "
    } else {
        ""
    };
    match (args.crit_over, args.crit_under) {
        (Some(over), Some(under)) => println!("({}between {} and {})", prefix, over, under),
        (Some(crit_over), None) => println!("({}less or equal to {})", prefix, crit_over),
        (None, Some(crit_under)) => println!("({}greater or equal to {})", prefix, crit_under),
        (None, None) => unreachable!("unexpected got no over under"),
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

    if args.kill_matching.is_some() {
        let signal = args.kill_matching.unwrap().0;
        let errors: Vec<_> = matches
            .iter()
            .map(|&(pid, _)| kill(*pid, signal))
            .filter(|result| result.is_err())
            .collect();
        if !errors.is_empty() {
            println!("INFO: There were {} errors killing processes", errors.len());
        }
    }
    if args.kill_matching_parents.is_some() {
        let signal = args.kill_matching_parents.unwrap().0;
        let mut parents = HashSet::new();
        let errors: Vec<_> = matches
            .iter()
            .map(|&(_, process)| process.stat.ppid)
            .filter(|ppid| parents.insert(*ppid))
            .map(|ppid| kill(ppid, signal))
            .filter(|result| result.is_err())
            .collect();
        if !errors.is_empty() {
            println!("INFO: There were {} errors killing processes", errors.len());
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
    re: &Option<Regex>,
    states: &[State],
    procs: &'m ProcMap,
) -> Vec<(&'m Pid, &'m Process)> {
    let me = getpid();
    let parent = getppid();

    debug!("filtering for procs that match: {:?} {:?}", re, states);

    procs
        .iter()
        .filter(|&(pid, _process)| *pid != me && *pid != parent)
        .filter(|&(_pid, process)| states.is_empty() || states.contains(&process.stat.state))
        .filter(|&(_pid, process)| {
            re.as_ref()
                .map(|re| {
                    let cmdline = process.useful_cmdline();
                    let is_match = re.is_match(&cmdline);
                    trace!("is_match={} process={}", is_match, cmdline);
                    is_match
                })
                .unwrap_or(true)
        })
        .collect()
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
    fn validate_parse_zombies_and_pattern() {
        let args = Args::from_iter(["c-p", "--state", "zombie", "--", "some.*proc"].into_iter());
        assert_eq!(args.states, [State::Zombie]);
        let args = Args::from_iter(["c-p", "--state=zombie", "some.*proc"].into_iter());
        assert_eq!(args.states, [State::Zombie]);
        let args = Args::from_iter(["c-p", "so.*proc", "--state", "zombie", "waiting"].into_iter());
        assert_eq!(args.states, [State::Zombie, State::Waiting]);
    }

    // Waiting for structopt 0.2.8 to be released with the from_iter_safe method
    // #[test]
    // #[should_panic]
    // fn regexes_must_be_valid() {
    //     Args::from_iter_safe(["c-p", "--crit-over=5", "["].into_iter()).unwrap();
    // }

    #[test]
    fn filter_procs_handles_patterns() {
        let mut procs = vec![Process::default(); 5];
        procs[2]
            .cmdline
            .raw
            .append(&mut vec!["hello".into(), "jar".into()]);
        let proc_map = vec_to_procmap(procs);
        let filtered = filter_procs(&regex("llo.*ar"), &[], &proc_map);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_procs_handles_single_state() {
        let mut procs = vec![Process::default(); 5];
        procs[2].stat.state = State::Zombie;
        let proc_map = vec_to_procmap(procs);
        let filtered = filter_procs(&None, &[State::Zombie], &proc_map);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_procs_handles_multiple_states() {
        let mut procs = vec![Process::default(); 5];
        procs[2].stat.state = State::Zombie;
        procs[3].stat.state = State::Stopped;
        let proc_map = vec_to_procmap(procs);

        let filtered = filter_procs(&None, &[State::Zombie], &proc_map);
        assert_eq!(filtered.len(), 1);

        let filtered = filter_procs(&None, &[State::Zombie, State::Stopped], &proc_map);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_procs_handles_multiple_states_and_regex() {
        let mut procs = vec![Process::default(); 5];
        procs[2].stat.state = State::Zombie;
        procs[2].cmdline.raw.push("example".into());

        procs[3].stat.state = State::Stopped;
        let proc_map = vec_to_procmap(procs.clone());

        let filtered = filter_procs(&regex("exa"), &[], &proc_map);
        assert_eq!(filtered.len(), 1);

        let filtered = filter_procs(&regex("exa"), &[State::Zombie, State::Stopped], &proc_map);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].1, &procs[2]);

        let filtered = filter_procs(&regex("exa"), &[State::Stopped], &proc_map);
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

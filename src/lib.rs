//! Iron Fan: strongly typed Sensu handlers
//!
//! The goal is to make it easy to transplant the majority of the
//! [`sensu-community-plugins`](https://github.com/sensu/sensu-community-plugins)
//! repo over to Rust, supporting the idioms provided there but strongly typed
//! and fast to execute, because your monitoring system shouldn't go down
//! because you mistyped `do_alret('critical')`.
//!
//! Expected use: write a bin that uses iron_fan::init(), which will populate
//! an `Event` and run filters on it:
//!
//! ```rust
//! extern crate turbine;
//!
//! fn main() {
//!     // parse stdin into an iron_fan::Event
//!     let event = iron_fan::init();
//!     match event {
//!         Ok(event) => { /* do_stuff_with(event) */ },
//!         Err(iron_fan::InitError(msg)) => println!("{}", msg),
//!         _ => { /* handle other errors */ }
//!     };
//! }
//! ```
//!
//! This is my first serious experimentation with Rust, comments apprectiated.
//! Implementation issues that I would like to fix:
//!
//! * `Event`s, `Client`s and `Check`s are manually parsed, insted of being
//!   decoded in some way

extern crate rustc_serialize;
extern crate url;
extern crate hyper;

pub use self::init::{
    init,
    read_event,
    Event,
    Client,
    Check,
    SensuError,
};

pub use filter::{
    Filter,
    run_filters,
    run_filters_or_die,
};

mod init;
pub mod filter;
pub mod utils;

// XXX: these will be gotten from settings, when that's implemented
struct Defaults {
    refresh: u64,
    interval: u64,
}

fn defaults() -> Defaults {
    Defaults {
        refresh: 1800,
        interval: 60,
    }
}

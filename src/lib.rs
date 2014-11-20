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
//! extern crate iron_fan;
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

#![feature(macro_rules)]
extern crate serialize;
extern crate libc;

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

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

use serialize::json;
use serialize::json::{
    Boolean,
    F64,
    I64,
    Json,
    List,
    Null,
    String,
    U64,
};
use std::io;
use std::from_str::from_str;

/// An Event: [The event data](http://sensuapp.org/docs/latest/event_data)
/// formatted as a struct.
#[deriving(Show, PartialEq)]
pub struct Event {
    client: Client,
    check: Check,
    occurrences: u64,
    action: Option<String>,
}

/// The configuration of the client that sent this event
#[deriving(Show, PartialEq)]
pub struct Client {
    name: String,
    address: String,
    subscriptions: Vec<String>,
    timestamp: i64,
    additional: Json
}

/// The client-side definition of the check plus some extra status. See the
/// [check documentation](http://sensuapp.org/docs/latest/checks) for details.
#[deriving(Show, PartialEq)]
pub struct Check {
    // client-configured things
    name: String,
    command: String,
    subscribers: Vec<String>,
    interval: i64,
    additional: Option<Json>,

    // Results, added by Sensu
    handlers: Option<Vec<String>>,
    handler: Option<String>,
    issued: i64,
    output: String,
    status: i8,
    history: Vec<String>,
    flapping: bool,
}

#[deriving(Show, PartialEq)]
pub enum SensuError {
    InitError(String),
    EventError(String)
}

pub type SensuResult<T> = Result<T, SensuError>;

/// Extract a generic JS object, given a type
/// Early return an error if it can't be extracted
macro_rules! jk(
    ($j:ident->$k:expr $p:ident) => (
        match $j.find($k) {
            Some(value) => match value {
                &$p(ref v) => v.clone(),
                _ => return Err(EventError(format!("Wrong type for '{}': {}", $k, value)))
            },
            None => return Err(EventError(format!("couldn't find {} in event", $k)))
        }
    )
)
/// Extract any kind of number as an i64
/// Early return an error if it can't be extracted
macro_rules! jki {
    ($event:ident, $field:expr) => {
        match $event.find($field) {
            Some(value) => match value {
                &I64(ref v) => v.clone(),
                &U64(ref v) => v.clone() as i64,
                &F64(ref v) => v.clone() as i64,
                _ => return Err(EventError(format!("Wrong type for '{}': {}", $field, value)))
            },
            None => return Err(EventError(format!("couldn't find '{}' in event", $field)))
        };
    }
}

/// Read stdin, and parse it into an Event
pub fn init() -> SensuResult<Event> {
    match io::stdin().read_to_end() {
        Ok(input) => read_event(input.to_string().as_slice()),
        Err(e) => Err(InitError(format!("couldn't read stdin: {}", e)))
    }
}

/// Parse an &str into an Event, making sure that all the types match up
pub fn read_event(input: &str) -> SensuResult<Event> {
    let event = match json::from_str(input) {
        Ok(v) => v,
        Err(e) => return Err(InitError(format!("couldn't parse json: {}", e)))
    };

    let client = try!(read_client(&event));
    let check = try!(read_check(&event));
    let action = match event.find("action") {
        Some(action) => match *action {
            String(ref a) => Some(a.clone()),
            _ => return Err(InitError(format!("Wrong type for action: {}", action)))
        },
        None => None
    };
    let occurrences = match event.find("occurrences") {
        Some(occurrences) => match *occurrences {
            U64(o) => o,
            I64(o) => o as u64,
            F64(o) => o as u64,
            String(ref o) => try!(from_str::<u64>(o.as_slice()).ok_or(
                InitError(format!("Unable to parse number from occurrences: {}", occurrences)))),
            _ => return Err(InitError(format!("Wrong type for occurrences: {}", occurrences)))
        },
        None => return Err(InitError(format!("No occurrences in event data")))
    };
    Ok(Event{
        client: client,
        check: check,
        action: action,
        occurrences: occurrences,
    })
}


fn extract_list_of_strings(list: &Json, field_name: &str) -> SensuResult<Vec<String>> {
    match *list {
        List(ref subs) => {
            let mut result = Vec::new();
            for sub in subs.iter() {
                match *sub {
                    String(ref s) => result.push(s.clone()),
                    _ => return Err(InitError(format!("Wrong type in {}: {}",
                                                      field_name, sub)))
                }
            }
            Ok(result)
        },
        _ => return Err(InitError(format!("{} is not a list ({})", field_name, list)))
    }
}

fn find_list_of_strings(obj: &Json, field_name: &str) -> SensuResult<Vec<String>> {
    let contained = obj.find(field_name);
    println!("{} is {}", field_name, contained)
    let result = match contained {
        Some(subs) => {
            try!(extract_list_of_strings(subs, field_name))
        },
        None => return Err(InitError(format!("Couldn't build {} for check", field_name)))
    };
    Ok(result)
}

fn read_client(input: &Json) -> SensuResult<Client> {
    let event = match input.find("client") {
        Some(client) => client,
        None => return Err(InitError("No client in event".into_string()))
    };

    let unverified_subs = jk!(event->"subscriptions" List);
    let mut subs: Vec<String> = Vec::new();
    for sub in unverified_subs.iter() {
        match *sub {
            json::String(ref s) => subs.push(s.clone()),
            _ => return Err(EventError(format!("Invalid subscription type {}", sub)))
        }
    }

    let client = Client {
        name: jk!(event->"name" String),
        address: jk!(event->"address" String),
        subscriptions: subs,
        timestamp: jki!(event, "timestamp"),
        additional: Null
    };
    Ok(client)
}

fn read_check(event: &Json) -> SensuResult<Check> {
    let check = match event.find("check") {
        Some(check) => check,
        None => return Err(InitError("No check in event".into_string()))
    };

    let subscribers = try!(find_list_of_strings(check, "subscribers"));

    Ok(Check {
        name: jk!(check->"name" String),
        issued: jki!(check, "issued"),
        output: jk!(check->"output" String),
        status: jki!(check, "status") as i8,
        command: jk!(check->"command" String),
        subscribers: subscribers,
        interval: jki!(check, "interval"),
        handler: match check.find("handler") {
            Some(handler) => match *handler {
                String(ref h) => Some(h.clone()),
                _ => return Err(InitError(format!("Wrong type for handler ({})", handler)))
            },
            None => None
        },
        handlers: match check.find("handlers") {
            Some(handlers) => Some(try!(extract_list_of_strings(handlers, "handlers"))),
            None => None
        },
        history: try!(find_list_of_strings(check, "history")),
        flapping: jk!(check->"flapping" Boolean),
        additional: None
    })
}


#[cfg(test)]
mod build_objects {
    use serialize::json;
    use super::{read_event, read_client, read_check, Check};

    #[test]
    fn can_build_event() {
        let event = r#"{
            "occurrences": 1,
            "action": "create",
            "client": {
                "name": "hello",
                "address": "192.168.1.1",
                "subscriptions": ["one", "two"],
                "timestamp": 127897
            },
            "check": {
                "name": "test-check",
                "issued": 1416069607,
                "output": "we have output",
                "status": 0,
                "command": "echo 'we have output'",
                "subscribers": ["examples", "tests"],
                "interval": 60,
                "handler": "default",
                "history": ["0", "0", "0"],
                "flapping": false,
                "additional": null
            }
        }"#;
        match read_event(event) {
            Ok(e) => println!("Parsed event: {}", e),
            Err(e) => panic!("ERROR: {}", e)
        };
    }

    #[test]
    fn can_build_client() {
        let event = json::from_str(r#"{
            "client": {
                "name": "hello",
                "address": "192.168.1.1",
                "subscriptions": ["one", "two"],
                "timestamp": 127897
            }
        }"#).unwrap();
        let client = read_client(&event);
        match client {
            Ok(cl) => println!("Parsed: {}", cl),
            Err(e) => panic!("ERROR: {}", e)
        }
    }

    #[test]
    fn can_build_check() {
        let event = json::from_str(r#"{
            "check": {
                "name": "test-check",
                "issued": 1416069607,
                "output": "we have output",
                "status": 0,
                "command": "echo 'we have output'",
                "subscribers": ["examples", "tests"],
                "interval": 60,
                "handler": "default",
                "history": ["0", "0", "0"],
                "flapping": false,
                "additional": null
            }
        }"#).unwrap();
        match read_check(&event) {
            Ok(check) => {
                println!("Parsed: {}", check);
                assert_eq!(check, Check {
                    name: "test-check".into_string(),
                    issued: 1416069607i64,
                    output: "we have output".into_string(),
                    status: 0,
                    command: "echo 'we have output'".into_string(),
                    subscribers: vec!("examples".into_string(),
                                      "tests".into_string()),
                    interval: 60,
                    handler: Some("default".into_string()),
                    handlers: None,
                    history: Vec::from_fn(3, |_| "0".into_string()),
                    flapping: false,
                    additional: None });
            },
            Err(e) => panic!("ERROR: {}", e)
        }
    }

    #[test]
    fn can_build_check_with_handlers() {
        let event = json::from_str(r#"{
            "check": {
                "name": "test-check",
                "issued": 1416069607,
                "output": "we have output",
                "status": 0,
                "command": "echo 'we have output'",
                "subscribers": ["examples", "tests"],
                "interval": 60,
                "handlers": ["default", "pagerduty"],
                "history": ["0", "0", "0"],
                "flapping": false,
                "additional": null
            }
        }"#).unwrap();
        match read_check(&event) {
            Ok(cl) => println!("Parsed: {}", cl),
            Err(e) => panic!("ERROR: {}", e)
        }
    }
}

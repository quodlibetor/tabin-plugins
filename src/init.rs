use hyper;
use serialize::json;
use serialize::json::Json;
use serialize::json::Json::{
    Boolean,
    F64,
    I64,
    Array,
    Null,
    String,
    U64,
};
use std::io;
use std::str::from_str;
use std::error;

use SensuError::{InitError, EventError};
use defaults;

/// An Event: [The event data](http://sensuapp.org/docs/latest/event_data)
/// formatted as a struct.
#[deriving(Show, PartialEq)]
pub struct Event {
    /// Client configuration
    pub client: Client,
    /// Check Configuration
    pub check: Check,
    /// Count of the number of times this event has been going continuosly
    pub occurrences: u64,
    /// One of "create", "resolve"
    pub action: Option<String>,
}

/// The configuration of the client that sent this event
#[deriving(Show, PartialEq)]
pub struct Client {
    /// The name the client registered as
    pub name: String,
    /// The address registered by the client
    pub address: String,
    /// The subscriptions the client registered for
    pub subscriptions: Vec<String>,
    pub timestamp: i64,
    /// **Everything else**
    pub additional: Json
}

/// The client-side definition of the check plus some extra status. See the
/// [check documentation](http://sensuapp.org/docs/latest/checks) for details.
#[deriving(Show, PartialEq)]
pub struct Check {
    /* ====(  client-configured things  )==== */
    /// Name of the check (from check definition)
    pub name: String,
    /// Command being executed (from check definition)
    pub command: String,
    /// Subscription channel for the check (from check definition)
    pub subscribers: Vec<String>,
    /// How often to run the check (from check definition)
    pub interval: u64,
    /// Array of handlers to run when check results in an event (from check definition)
    pub handlers: Option<Vec<String>>,
    /// Single handler to run when check results in an event (from check definition)
    pub handler: Option<String>,

    /* ====(  Semi-standard client configs  )==== */
    /// Whether or not to alert (custom for sensu-handler)
    pub alert: Option<bool>,
    /// Minimum number of occurrences required to alert (custom for sensu-handler)
    pub occurrences: Option<u64>,
    /// Interval to run handlers (custom for sensu-handler)
    pub refresh: u64,

    /* ====(  Results, added by Sensu  )==== */
    /// When the check was issued (added by Sensu daemon)
    pub issued: i64,
    /// Check command output (added by Sensu daemon)
    pub output: String,
    /// Check exit status (added by Sensu daemon)
    pub status: i8,
    /// History of exit statuses (added by Sensu server)
    pub history: Vec<String>,
    /// Whether the check is currently flapping (added by Sensu server)
    pub flapping: bool,

    /// **Everything else**
    pub additional: Option<Json>,
}

#[deriving(Show, PartialEq)]
pub enum SensuError {
    InitError(String),
    EventError(String),
    ParseError(String),
    HttpError(hyper::HttpError)
}

pub type SensuResult<T> = Result<T, SensuError>;

impl<E: error::Error> error::FromError<E> for SensuError {
    fn from_error(_: E) -> SensuError {
        super::init::SensuError::ParseError("bad".into_string())
    }
}


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

macro_rules! find_default(
    ($j:ident, $p:ident($k:expr), $default:expr) => (
        match $j.find($k) {
            Some(value) => match value {
                &$p(ref v) => v.clone(),
                _ => return Err(EventError(format!("Wrong type for '{}': {}", $k, value)))
            },
            None => $default
        }
    )
)

/// Extract any kind of number as an i64
/// Early return an error if it can't be extracted
macro_rules! jki {
    ($event:ident, $field:expr) => {
        match $event.find($field) {
            Some(value) => match value {
                &Json::I64(ref v) => v.clone(),
                &Json::U64(ref v) => v.clone() as i64,
                &Json::F64(ref v) => v.clone() as i64,
                _ => return Err(EventError(format!("Wrong type for '{}': {}", $field, value)))
            },
            None => return Err(EventError(format!("couldn't find '{}' in event", $field)))
        };
    }
}

macro_rules! find {
    ($event:ident, $pat:ident($field:expr)) => (
        match $event.find($field) {
            Some(value) => match *value {
                $pat(v) => Some(v),
                _ => return Err(EventError(format!("Wrong type for '{}': '{}'", $field, value)))
            },
            None => None
        }
    )
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
            Json::String(ref a) => Some(a.clone()),
            _ => return Err(InitError(format!("Wrong type for action: {}", action)))
        },
        None => None
    };
    let occurrences = match event.find("occurrences") {
        Some(occurrences) => match *occurrences {
            Json::U64(o) => o,
            Json::I64(o) => o as u64,
            Json::F64(o) => o as u64,
            Json::String(ref o) => try!(from_str::<u64>(o.as_slice()).ok_or(
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
        Json::Array(ref subs) => {
            let mut result = Vec::new();
            for sub in subs.iter() {
                match *sub {
                    Json::String(ref s) => result.push(s.clone()),
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

    let unverified_subs = jk!(event->"subscriptions" Array);
    let mut subs: Vec<String> = Vec::new();
    for sub in unverified_subs.iter() {
        match *sub {
            Json::String(ref s) => subs.push(s.clone()),
            _ => return Err(EventError(format!("Invalid subscription type {}", sub)))
        }
    }

    let client = Client {
        name: jk!(event->"name" String),
        address: jk!(event->"address" String),
        subscriptions: subs,
        timestamp: jki!(event, "timestamp"),
        additional: Json::Null
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

        alert: find!(check, Boolean("alert")),
        occurrences: find!(check, U64("occurrences")),

        interval: find_default!(check, U64("interval"), defaults().interval),
        refresh: find_default!(check, U64("refresh"), defaults().refresh),

        handler: match check.find("handler") {
            Some(handler) => match *handler {
                Json::String(ref h) => Some(h.clone()),
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
    use super::{read_event, read_check, read_client, Check};

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
                "refresh": 1800,
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
                    alert: None,
                    occurrences: None,
                    interval: 60,
                    refresh: 1800,
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
    fn can_build_check_with_alert() {
        let event = json::from_str(r#"{
            "check": {
                "name": "test-check",
                "issued": 1416069607,
                "output": "we have output",
                "status": 0,
                "command": "echo 'we have output'",
                "subscribers": ["examples", "tests"],
                "interval": 60,
                "refresh": 1800,
                "alert": false,
                "occurrences": 5,
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

                    alert: Some(false),
                    occurrences: Some(5),

                    refresh: 1800,
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

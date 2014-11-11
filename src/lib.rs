#![feature(macro_rules)]
extern crate serialize;
extern crate chrono;

//use std::collections::HashMap;
use serialize::json;
use serialize::json::{
    Json,
    String,
    F64,
    U64,
    I64,
    List,
    Null,
};
use std::io;
use chrono::NaiveDateTime;

#[deriving(Show)]
pub struct Event {
    client: Client,
    check: Check,
    occurences: Vec<i32>,
    action: String,
}

#[deriving(Show)]
pub struct Client {
    name: String,
    address: String,
    subscriptions: Vec<String>,
    timestamp: NaiveDateTime,
    additional: Json
}

#[deriving(Show)]
pub struct Check {
    name: String,
    issued: NaiveDateTime,
    output: String,
    status: i8,
    command: String,
    subscribers: Vec<String>,
    interval: i32,
    handler: Option<String>,
    handlers: Option<Vec<String>>,
    history: Vec<i8>,
    flapping: bool,
    additional: Json
}

#[deriving(Show)]
pub enum SensuError {
    InitError(String),
    EventError(String)
}

macro_rules! jk(
    ($j:ident->$k:expr $p:ident) => (
        match $j.find($k) {
            Some(value) => match value {
                &$p(ref v) => v.clone(),
                _ => return Err(EventError(format!("Wrong type for key: {}", value)))
            },
            None => return Err(EventError(format!("couldn't find {} in event", $k)))
        }
    )
)

pub fn read_stdin() -> Result<String, SensuError> {
    match io::stdin().read_to_end() {
        Ok(input) => Ok(input.to_string()),
        Err(e) => return Err(InitError(format!("couldn't read stdin: {}", e)))
    }
}

pub fn read_event(input: &str) -> Result<Client, SensuError> {
    let event = match json::from_str(input) {
        Ok(v) => v,
        Err(e) => return Err(InitError(format!("couldn't parse json: {}", e)))
    };

    let client = try!(read_client(event));
    Ok(client)
}

fn read_client(input: Json) -> Result<Client, SensuError> {
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
        timestamp: NaiveDateTime::from_num_seconds_from_unix_epoch(
            match event.find("timestamp") {
                Some(value) => match value {
                    &I64(ref v) => v.clone(),
                    &U64(ref v) => v.clone() as i64,
                    &F64(ref v) => v.clone() as i64,
                    _ => return Err(EventError(format!("Wrong type for key: {}", value)))
                },
                None => return Err(EventError("couldn't find timestamp in event".into_string()))
            }, 0),
        additional: Null
    };
    Ok(client)
}



#[cfg(test)]
mod build_objects {
    use serialize::json;
    use super::read_client;

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
        let client = read_client(event);
        match client {
            Ok(cl) => println!("Parsed: {}", cl),
            Err(e) => panic!("ERROR: {}", e)
        }
    }
}

extern crate iron_fan;
extern crate serialize;
use serialize::json::Json;

fn main() {
    let event = iron_fan::Event {
        occurrences: 1,
        action: Some("create".into_string()),
        client: iron_fan::Client {
            name: "hello".into_string(),
            address: "192.168.1.1".into_string(),
            subscriptions: vec!("one".into_string()),
            timestamp: 127897,
            additional: Json::Null
        },
        check: iron_fan::Check {
            name: "test-check".into_string(),
            issued: 1416069607,
            output: "we have output".into_string(),
            status: 0,
            command: "echo 'we have output'".into_string(),
            subscribers: vec!("examples".into_string(), "tests".into_string()),
            interval: 60,
            handler: None,
            handlers: None,
            alert: None,
            occurrences: Some(4),
            refresh: 60,
            history: vec!("0".into_string(), "0".into_string(), "0".into_string()),
            flapping: false,
            additional: None
        }
    };
    iron_fan::filter::run_filters_or_die(&event);
}

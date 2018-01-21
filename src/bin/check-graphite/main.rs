extern crate chrono;
#[macro_use]
extern crate clap;
extern crate itertools;
extern crate reqwest;
extern crate url;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tabin_plugins;

mod args;
mod assertions;
mod graphite;

use std::cmp::max;
use tabin_plugins::Status;

use args::Args;
use graphite::{fetch_data, filter_to_with_data};

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args = Args::parse();
    let data = match fetch_data(
        &args.url,
        &args.path,
        args.window,
        args.start_at,
        args.retries,
        &args.graphite_error,
        args.print_url,
    ) {
        Ok(data) => data,
        Err(e) => {
            println!("{}", e);
            args.graphite_error.exit();
        }
    };

    let filtered = filter_to_with_data(&args.path, data.result, args.no_data);
    let with_data = match filtered {
        Ok(data) => data,
        Err(status) => {
            println!("INFO: Full query: {}", data.url);
            status.exit();
        }
    };

    let mut status = Status::Ok;
    for assertion in args.assertions {
        status = max(
            status,
            do_check(
                &with_data,
                &assertion.operator,
                assertion.op_is_negated,
                assertion.threshold,
                assertion.point_assertion,
                assertion.failure_status,
            ),
        );
    }
    status.exit();
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    // A couple helper methods for tests in other modules

    use chrono::naive::NaiveDateTime;
    use serde_json;

    use graphite::{DataPoint, GraphiteData};

    pub(crate) fn deser(s: &str) -> Vec<GraphiteData> {
        let result = serde_json::from_str(s);
        result.unwrap()
    }

    fn dt(t: i64) -> NaiveDateTime {
        NaiveDateTime::from_timestamp(t, 0)
    }

    pub(crate) fn valid_data_from_json_two_sets() -> Vec<GraphiteData> {
        vec![
            GraphiteData {
                points: vec![
                    DataPoint {
                        val: Some(1_f64),
                        time: dt(11150),
                    },
                    DataPoint {
                        val: None,
                        time: dt(11160),
                    },
                    DataPoint {
                        val: Some(3_f64),
                        time: dt(11170),
                    },
                ],
                target: "test.path.some-data".to_owned(),
            },
        ]
    }
}

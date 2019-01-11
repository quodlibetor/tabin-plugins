extern crate chrono;
#[macro_use]
extern crate clap;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tabin_plugins;
extern crate url;

mod args;
mod assertions;
mod graphite;

use std::cmp::max;
use tabin_plugins::Status;

use args::Args;
use graphite::{fetch_data, GraphiteResponse};

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args = Args::parse();
    let mut data = match fetch_data(
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

    bail_if_no_data(&mut data, &args.path, args.no_data);

    let mut status = Status::Ok;
    for assertion in args.assertions {
        status = max(status, assertion.check(&data.result));
    }
    status.exit();
}

/// Check if we have any graphite data
///
/// If there is nothing to do, exit
fn bail_if_no_data(response: &mut GraphiteResponse, path: &str, no_data_status: Status) {
    let mut bail = false;
    if response.result.is_empty() {
        println!(
            "{}: Graphite returned no matching series for pattern '{}'",
            no_data_status, path
        );
        bail = true;
    }
    let original_len = response.result.len();
    response.filter_to_series_with_data();
    if response.result.is_empty() {
        println!(
            "{}: Graphite found {} series but returned only null datapoints for them",
            no_data_status, original_len
        );
        bail = true;
    }

    if bail {
        println!("INFO: Full query: {}", response.url);
        no_data_status.exit();
    }
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
        vec![GraphiteData {
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
        }]
    }
}

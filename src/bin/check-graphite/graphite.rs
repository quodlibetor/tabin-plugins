//! Interact with graphite
//!
//! This module defines functions that can fetch data from
//! graphite, and the types that they return.
//!
//! The two top-level items in here

use std::error::Error;
use std::fmt;
use std::io::{self, Read};
use std::thread::sleep;
use std::time::Duration;

use chrono::naive::serde::ts_seconds::deserialize as from_ts_seconds;
use chrono::naive::NaiveDateTime;
use reqwest;
use reqwest::Error as ReqwestError;
use serde_json;
use tabin_plugins::Status;

/// The result of `fetch_data`
pub struct GraphiteResponse {
    pub result: Vec<GraphiteData>,
    pub url: reqwest::Url,
}

impl GraphiteResponse {
    /// Mutate self to only contain series that have at least one existing
    /// datapoint
    pub fn filter_to_series_with_data(&mut self) {
        self.result.retain(|gd| {
            gd.points
                .iter()
                .filter(|point| !point.val.is_none())
                .count()
                > 0
        })
    }
}

/// All the data for one fully-resolved target
///
/// Any given graphite api call can result in getting data for multiple targets
#[derive(PartialEq, Debug, Deserialize)]
pub struct GraphiteData {
    #[serde(rename = "datapoints")]
    pub points: Vec<DataPoint>,
    pub target: String,
}

impl GraphiteData {
    /// References to the points that exist and do not satisfy the comparator
    // comparator is a box closure, which is not allows in map_or
    pub(crate) fn invalid_points(&self, comparator: &Box<Fn(f64) -> bool>) -> Vec<&DataPoint> {
        self.points
            .iter()
            .filter(|p| p.val.map_or(false, |v| comparator(v)))
            .collect()
    }

    /// Get only invalid points from the end of the list
    // comparator is a box closure, which is not allows in map_or
    pub(crate) fn last_invalid_points(
        &self,
        n: usize,
        comparator: &Box<Fn(f64) -> bool>,
    ) -> Vec<&DataPoint> {
        self.points
            .iter()
            .rev()
            .filter(|p| p.val.is_some())
            .take(n)
            .filter(|p| p.val.map_or(false, |v| comparator(v)))
            .collect()
    }
}

/// Represent the data that we received after some filtering operation
#[derive(Debug, PartialEq)]
pub struct FilteredGraphiteData<'a> {
    pub original: &'a GraphiteData,
    pub points: Vec<&'a DataPoint>,
}

impl<'a> FilteredGraphiteData<'a> {
    /// The number of points that we have
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// If there are any points in the filtered graphite data
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The percent of the original points that were included by the filter
    ///
    /// This only includes the original points that actually have data
    pub fn percent_matched(&self) -> f64 {
        (self.len() as f64
            / self
                .original
                .points
                .iter()
                .filter(|point| point.val.is_some())
                .count() as f64)
            * 100.0
    }
}

/// One of the datapoints that graphite has returned.
///
/// Graphite always returns all values in its time range, even if it hasn't got
/// any data for them, so the val might not exist.
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DataPoint {
    pub val: Option<f64>,
    #[serde(deserialize_with = "from_ts_seconds")]
    pub time: NaiveDateTime,
}

impl fmt::Display for DataPoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} (at {})",
            self.val.map_or("null".into(), |v| {
                format!(
                    "{:.*}",
                    // the number of points past the dot to show
                    // Don't show any if it's an integer
                    if v.round() == v { 0 } else { 2 },
                    v
                )
            }),
            self.time.format("%H:%Mz")
        )
    }
}

pub struct GraphiteIterator {
    current: usize,
    back: usize,
    data: GraphiteData,
}

impl Iterator for GraphiteIterator {
    type Item = DataPoint;
    fn next(&mut self) -> Option<DataPoint> {
        self.current += 1;
        self.data.points.get(self.current - 1).cloned()
    }
}

impl IntoIterator for GraphiteData {
    type Item = DataPoint;
    type IntoIter = GraphiteIterator;
    fn into_iter(self) -> Self::IntoIter {
        GraphiteIterator {
            current: 0,
            back: self.points.len(),
            data: self,
        }
    }
}

impl DoubleEndedIterator for GraphiteIterator {
    fn next_back(&mut self) -> Option<DataPoint> {
        self.back -= 1;
        self.data.points.get(self.back).cloned()
    }
}

pub enum GraphiteError {
    HttpError(ReqwestError),
    JsonError(String),
    IoError(String),
}

impl GraphiteError {
    pub fn short_display(&self) -> String {
        match *self {
            GraphiteError::HttpError(ref e) => e.to_string(),
            GraphiteError::JsonError(_) => "Error parsing json".to_owned(),
            GraphiteError::IoError(_) => "Error reading stream from graphite".to_owned(),
        }
    }
}

impl From<ReqwestError> for GraphiteError {
    fn from(e: ReqwestError) -> Self {
        GraphiteError::HttpError(e)
    }
}

impl From<io::Error> for GraphiteError {
    fn from(e: io::Error) -> Self {
        GraphiteError::IoError(e.description().to_owned())
    }
}

impl fmt::Display for GraphiteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            GraphiteError::HttpError(ref e) => e.fmt(f),
            GraphiteError::JsonError(ref e) => write!(f, "{}", e),
            GraphiteError::IoError(ref e) => write!(f, "{}", e),
        }
    }
}

/// Load data from graphite
///
/// Retry until success or exit the script
pub fn fetch_data(
    url: &str,
    target: &str,
    window: i64,
    start_at: i64,
    retries: u8,
    graphite_error: &Status,
    print_url: bool,
) -> Result<GraphiteResponse, String> {
    let mut attempts = 0;
    let mut retry_sleep = 2000;
    loop {
        match get_graphite(url, target, window, start_at, print_url, graphite_error) {
            Ok(s) => {
                return Ok(s);
            }
            Err(e) => {
                print!("Error for {}: {}. ", url, e.short_display());
                if attempts < retries {
                    println!("Retrying in {}s.", retry_sleep / 1000);
                    attempts += 1;
                    sleep(Duration::from_millis(retry_sleep));
                    retry_sleep *= 2;
                    continue;
                } else {
                    println!("\nFull error: {}", e);
                    println!("Giving up after {} attempts.", retries + 1);
                    graphite_error.exit();
                }
            }
        }
    }
}

/// Fetch data from graphite
///
/// Returns a tuple of (full request path, string that graphite returned), or an error
#[cfg_attr(test, allow(dead_code))]
fn get_graphite(
    url: &str,
    target: &str,
    window: i64,
    start_at: i64,
    print_url: bool,
    graphite_error: &Status,
) -> Result<GraphiteResponse, GraphiteError> {
    let full_path = format!(
        "{}/render?target={}&format=json&from=-{}min&until=-{}min",
        url, target, window, start_at
    );
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    if print_url {
        println!("INFO: querying {}", full_path);
    }
    let mut result = c.get(&full_path).send()?;
    let mut s = String::new();
    result.read_to_string(&mut s)?;
    match serde_json::from_str(&s) {
        Ok(data) => Ok(GraphiteResponse {
            result: data,
            url: result.url().clone(),
        }),
        Err(e) => {
            if e.is_syntax() || e.is_data() {
                Err(GraphiteError::JsonError(format!(
                    "{}: Graphite returned invalid json:\n\
                     {}\n=========================\n\
                     The full url queried was: {}",
                    graphite_error,
                    s,
                    result.url()
                )))
            } else {
                Err(GraphiteError::JsonError(format!(
                    "{}: {}",
                    graphite_error, e
                )))
            }
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use super::*;

    use test::deser;

    fn json_three_sets_of_graphite_data() -> &'static str {
        r#"
        [
            {
                "datapoints": [],
                "target": "test.path.no-data"
            },
            {
                "datapoints": [[null, 10]],
                "target": "test.path.null-data"
            },
            {
                "datapoints": [[1, 50]],
                "target": "test.path.some-data"
            }
        ]
        "#
    }

    #[test]
    fn deserialization_works() {
        let result = deser(json_three_sets_of_graphite_data());
        assert_eq!(
            result,
            vec![
                GraphiteData {
                    points: vec![],
                    target: "test.path.no-data".into(),
                },
                GraphiteData {
                    points: vec![DataPoint {
                        val: None,
                        time: NaiveDateTime::from_timestamp(10, 0),
                    },],
                    target: "test.path.null-data".into(),
                },
                GraphiteData {
                    points: vec![DataPoint {
                        val: Some(1.0),
                        time: NaiveDateTime::from_timestamp(50, 0),
                    },],
                    target: "test.path.some-data".into(),
                },
            ]
        )
    }

    #[test]
    fn filter_to_series_with_data_retails_valid() {
        let mut raw = GraphiteResponse {
            result: deser(json_three_sets_of_graphite_data()),
            url: "https://blah".parse().unwrap(),
        };

        raw.filter_to_series_with_data();
        assert_eq!(raw.result.len(), 1, "we should strip empty GraphiteData");
    }
}

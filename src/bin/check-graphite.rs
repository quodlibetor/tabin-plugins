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

use std::cmp::max;
use std::error::Error;
use std::fmt;
use std::io::{self, Read};
use std::str::FromStr;
use std::time::Duration;
use std::thread::sleep;

use chrono::naive::NaiveDateTime;
use chrono::naive::serde::ts_seconds::deserialize as from_ts_seconds;
use reqwest::Error as ReqwestError;
use itertools::Itertools;

use tabin_plugins::Status;


/// One of the datapoints that graphite has returned.
///
/// Graphite always returns all values in its time range, even if it hasn't got
/// any data for them, so the val might not exist.
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct DataPoint {
    val: Option<f64>,
    #[serde(deserialize_with = "from_ts_seconds")] time: NaiveDateTime,
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

/// All the data for one fully-resolved target
#[derive(PartialEq, Debug, Deserialize)]
struct GraphiteData {
    #[serde(rename = "datapoints")] points: Vec<DataPoint>,
    target: String,
}

/// Represent the data that we received after some filtering operation
#[derive(Debug, PartialEq)]
struct FilteredGraphiteData<'a> {
    original: &'a GraphiteData,
    points: Vec<&'a DataPoint>,
}

impl GraphiteData {
    /// References to the points that exist and do not satisfy the comparator
    // comparator is a box closure, which is not allows in map_or
    fn invalid_points(&self, comparator: &Box<Fn(f64) -> bool>) -> Vec<&DataPoint> {
        self.points
            .iter()
            .filter(|p| p.val.map_or(false, |v| comparator(v)))
            .collect()
    }

    /// Get only invalid points from the end of the list
    // comparator is a box closure, which is not allows in map_or
    fn last_invalid_points(&self, n: usize, comparator: &Box<Fn(f64) -> bool>) -> Vec<&DataPoint> {
        self.points
            .iter()
            .rev()
            .filter(|p| p.val.is_some())
            .take(n)
            .filter(|p| p.val.map_or(false, |v| comparator(v)))
            .collect()
    }
}

impl<'a> FilteredGraphiteData<'a> {
    /// The number of points that we have
    fn len(&self) -> usize {
        self.points.len()
    }

    /// If there are any points in the filtered graphite data
    fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The percent of the original points that were included by the filter
    ///
    /// This only includes the original points that actually have data
    fn percent_matched(&self) -> f64 {
        (self.len() as f64
            / self.original
                .points
                .iter()
                .filter(|point| point.val.is_some())
                .count() as f64) * 100.0
    }
}

struct GraphiteIterator {
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

enum GraphiteError {
    HttpError(ReqwestError),
    JsonError(String),
    IoError(String),
}

impl GraphiteError {
    fn short_display(&self) -> String {
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
        url,
        target,
        window,
        start_at
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
        Err(e) => if e.is_syntax() || e.is_data() {
            Err(GraphiteError::JsonError(format!(
                "{}: Graphite returned invalid json:\n\
                 {}\n=========================\n\
                 The full url queried was: {}",
                graphite_error,
                s,
                result.url()
            )))
        } else {
            Err(GraphiteError::JsonError(
                format!("{}: {}", graphite_error, e),
            ))
        },
    }
}

struct GraphiteResponse {
    result: Vec<GraphiteData>,
    url: reqwest::Url,
}

/// Load data from graphite
///
/// Retry until success or exit the script
fn fetch_data(
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

/// Take an operator and a value and return a function that can be used in a
/// filter to return only values that *do not* satisfy the operator.
///
/// aka a function that returns only invalid numbers
///
/// Note: JSON considers all numbers either floats or arbitrary-precision
/// decimals, so we use f64. Comparing f64 directly equal with each other is dangerous.
///
/// We aren't ever actually doing math on floats, so even though decimal 0.1 !=
/// float 0.1, since we are always interpreting as f64 then we'll get the
/// "wrong" values and compare them to each other. That said, we should
/// probably use an epsilon in here.
fn operator_string_to_func(op: &str, op_is_negated: NegOp, val: f64) -> Box<Fn(f64) -> bool> {
    let comp: Box<Fn(f64) -> bool> = match op {
        "<" => Box::new(move |i: f64| i < val),
        "<=" => Box::new(move |i: f64| i <= val),
        ">" => Box::new(move |i: f64| i > val),
        ">=" => Box::new(move |i: f64| i >= val),
        "==" => Box::new(move |i: f64| i == val),
        "!=" => Box::new(move |i: f64| i != val),
        a => panic!("Bad operator: {}", a),
    };

    if op_is_negated == NegOp::Yes {
        Box::new(move |i: f64| !comp(i))
    } else {
        comp
    }
}

/// Make sure that at least one series has real data
///
/// This returns all points in the series that have any data -- it does not
/// filter out null points, it only filters out series that contain *only* null
/// points.
fn filter_to_with_data(
    path: &str,
    data: Vec<GraphiteData>,
    no_data_status: Status,
) -> Result<Vec<GraphiteData>, Status> {
    let matched_len = data.len();
    if data.is_empty() {
        println!(
            "{}: Graphite returned no matching series for pattern '{}'",
            no_data_status,
            path
        );
        return Err(no_data_status);
    }
    let series_with_data = data.into_iter()
        .filter(|series| {
            let null_point_count = series.points.iter().fold(0, |count, point| {
                if point.val.is_none() {
                    count + 1
                } else {
                    count
                }
            });
            !series.points.is_empty() && null_point_count != series.points.len()
        })
        .collect::<Vec<GraphiteData>>();

    if series_with_data.is_empty() {
        println!(
            "{}: Graphite found {} series but returned only null datapoints for them",
            no_data_status,
            matched_len
        );
        Err(no_data_status)
    } else {
        Ok(series_with_data)
    }
}

fn do_check(
    series_with_data: &[GraphiteData],
    op: &str,
    op_is_negated: NegOp,
    threshold: f64,
    error_condition: PointAssertion,
    status: Status,
) -> Status {
    let comparator = operator_string_to_func(op, op_is_negated, threshold);
    // We want to create a vec of series' that only have (existing) invalid
    // points. The first element is the original length of the vector of points
    let with_invalid = match error_condition {
        // Here, invalid points can exist anywhere
        PointAssertion::Ratio(error_ratio) => series_with_data
            .iter()
            .map(|series| {
                FilteredGraphiteData {
                    original: &series,
                    points: series.invalid_points(&comparator),
                }
            })
            .filter(|invalid| {
                if error_ratio == 0.0 {
                    !invalid.points.is_empty()
                } else {
                    let filtered = invalid.points.len() as f64;
                    let original = invalid.original.points.len() as f64;
                    filtered / original >= error_ratio
                }
            })
            .collect::<Vec<FilteredGraphiteData>>(),
        PointAssertion::Recent(count) => series_with_data
            .iter()
            .map(|ref series| {
                FilteredGraphiteData {
                    original: &series,
                    points: series.last_invalid_points(count, &comparator),
                }
            })
            .filter(|ref invalid| !invalid.is_empty())
            .collect::<Vec<(FilteredGraphiteData)>>(),
    };

    let nostr = if op_is_negated == NegOp::Yes {
        " not"
    } else {
        ""
    };
    if !with_invalid.is_empty() {
        match error_condition {
            PointAssertion::Ratio(ratio) => {
                if series_with_data.len() == with_invalid.len() {
                    if with_invalid.len() == 1 {
                        print!("{}: ", status)
                    } else if ratio == 0.0 {
                        println!(
                            "{}: All {} matched paths have invalid datapoints:",
                            status,
                            with_invalid.len()
                        )
                    } else {
                        println!(
                            "{}: All {} matched paths have at least {:.0}% invalid \
                             datapoints:",
                            status,
                            with_invalid.len(),
                            ratio * 100.0
                        )
                    }
                } else {
                    println!(
                        "{}: Of {} paths with data, {} have at least {:.1}% invalid \
                         datapoints:",
                        status,
                        series_with_data.len(),
                        with_invalid.len(),
                        ratio * 100.0
                    );
                }
                for series in &with_invalid {
                    let prefix = if with_invalid.len() == 1 {
                        ""
                    } else {
                        "       ->"
                    };
                    println!(
                        "{} {} has {} points ({:.1}%) that are{} {} {}: {}",
                        prefix,
                        series.original.target,
                        series.points.len(),
                        series.percent_matched(),
                        nostr,
                        op,
                        threshold,
                        series.points.iter().map(|gv| format!("{}", gv)).join(", ")
                    );
                }
            }
            PointAssertion::Recent(count) => {
                println!(
                    "{}: Of {} paths with data, {} have the last {} points invalid:",
                    status,
                    series_with_data.len(),
                    with_invalid.len(),
                    count
                );
                for series in &with_invalid {
                    let descriptor = if count == 1 { "point is" } else { "points are" };
                    println!(
                        "       -> {} last {} {}{} {} {}: {}",
                        series.original.target,
                        count,
                        descriptor,
                        nostr,
                        op,
                        threshold,
                        series.points.iter().map(|gv| format!("{}", gv)).join(", ")
                    );
                }
            }
        }
        Status::Critical
    } else {
        match error_condition {
            PointAssertion::Ratio(percent) => {
                let amount;
                if percent == 0.0 {
                    amount = "any".to_owned()
                } else {
                    amount = format!("at least {:.1}% of", percent * 100.0)
                }
                println!(
                    "OK: Found {} paths with data, none had {} datapoints{} {} {:.2}.",
                    series_with_data.len(),
                    amount,
                    nostr,
                    op,
                    threshold
                );
            }
            PointAssertion::Recent(count) => {
                println!(
                    "OK: Found {} paths with data, none had their last {} datapoints{} {} \
                     {}.",
                    series_with_data.len(),
                    count,
                    nostr,
                    op,
                    threshold
                );
            }
        }
        for series in series_with_data.iter() {
            println!(
                "    -> {}: {}",
                series.target,
                series.points.iter().map(|gv| format!("{}", gv)).join(", ")
            )
        }
        Status::Ok
    }
}

struct Args {
    url: String,
    path: String,
    assertions: Vec<Assertion>,
    window: i64,
    start_at: i64,
    retries: u8,
    graphite_error: Status,
    no_data: Status,
    print_url: bool,
}

static ASSERTION_EXAMPLES: &'static [&'static str] = &[
    "critical if any point is > 0",
    "critical if any point in at least 40% of \
     series is > 0",
    "critical if any point is not > 0",
    "warning if any point is == 9",
    "critical if all points are > 100.0",
    "critical if at least 20% of points are > \
     100",
    "critical if most recent point is > 5",
    "critical if most recent point in all \
     series are == 0",
];

fn parse_args() -> Args {
    let allowed_no_data = Status::str_values(); // block-local var for borrowck
    let args = clap::App::new("check-graphite")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Brandon W Maister <quodlibetor@gmail.com>")
        .about("Query graphite and exit based on predicates")
        .args_from_usage(
            "<URL>                 'The domain to query graphite. Must \
                                         include scheme (http/s)'
                                     <PATH> 'The graphite path to query. For example: \
                                         \"collectd.*.cpu\"'
                                     <ASSERTION>...        'The assertion to make against the PATH. \
                                         See Below.'
                                     -w --window=[MINUTES] 'How many minutes of data to test. \
                                         Default 10.'
                                     --window-start=[MINUTES_IN_PAST] 'How far back to start the window. \
                                         Default is now.'
                                    --retries=[COUNT]     'How many times to retry reaching graphite. \
                                        Default 4.'
                                    --print-url           'Unconditionally print the graphite \
                                        url queried'
                                    --verify-assertions   'Just check assertion syntax, do not \
                                        query urls'",
        )
        .arg(
            clap::Arg::with_name("NO_DATA_STATUS")
                .long("--no-data")
                .help(
                    "What to do with no data. Choices: ok, warn, critical, unknown.
                     This is the value to use for the assertion 'if all values
                     are null'
                     Default: warn.",
                )
                .takes_value(true)
                .possible_values(&allowed_no_data),
        )
        .arg(
            clap::Arg::with_name("GRAPHITE_ERROR_STATUS")
                .long("--graphite-error")
                .help(
                    "What to do with no data.
                     Choices: ok, warn, critical, unknown.
                     What to say if graphite returns a 500 or invalid JSON
                     Default: unknown.",
                )
                .takes_value(true)
                .possible_values(&allowed_no_data),
        )
        .after_help(
            format!(
                "About Assertions:

    Assertions look like 'critical if any point in any series is > 5'.

    They describe what you care about in your graphite data. The structure of
    an assertion is as follows:

        <errorkind> if <point spec> [in <series spec>] is|are [not] <operator> <threshold>

    Where:

        - `errorkind` is either `critical` or `warning`
        - `point spec` can be one of:
            - `any point`
            - `all points`
            - `at least <N>% of points`
            - `most recent point`
        - `series spec` (optional) can be one of:
            - `any series`
            - `all series`
            - `at least <N>% of series`
            - `not` is optional, and inverts the following operator
        - `operator` is one of: `==` `!=` `<` `>` `<=` `>=`
        - `threshold` is a floating-point value (e.g. 100, 78.0)

    Here are some example assertions:

        - `{}`\n",
                ASSERTION_EXAMPLES.join("`\n        - `")
            ).as_ref(),
        )
        .get_matches();

    let assertions = args.values_of("ASSERTION")
        .unwrap()
        .map(|assertion_str| match parse_assertion(assertion_str) {
            Ok(a) => a,
            Err(e) => {
                println!("Error `{}` in assertion `{}`", e, assertion_str);
                Status::Critical.exit();
            }
        })
        .collect();

    if args.is_present("verify-assertions") {
        Status::Ok.exit();
    }

    let start_offset = value_t!(args.value_of("MINUTES_IN_PAST"), i64).unwrap_or(0);
    let window = value_t!(args.value_of("MINUTES"), i64).unwrap_or(10);
    Args {
        url: args.value_of("URL").unwrap().to_owned(),
        path: args.value_of("PATH").unwrap().to_owned(),
        assertions: assertions,
        window: start_offset + window,
        start_at: start_offset,
        retries: value_t!(args.value_of("COUNT"), u8).unwrap_or(4),
        graphite_error: Status::from_str(
            args.value_of("GRAPHITE_ERROR_STATUS").unwrap_or("unknown"),
        ).unwrap(),
        no_data: Status::from_str(args.value_of("NO_DATA_STATUS").unwrap_or("warning")).unwrap(),
        print_url: args.is_present("print-url"),
    }
}

#[derive(Debug, PartialEq)]
struct Assertion {
    operator: String,
    op_is_negated: NegOp,
    threshold: f64,
    point_assertion: PointAssertion,
    series_ratio: f64,
    failure_status: Status,
}

enum AssertionState {
    /// Deciding if breaking this assertion means we go critical or warning
    Status,
    /// We're about to describe an assertion over points
    Points,
    /// We're about to describe an assertion over series
    Series,
    /// We're looking for an operator
    Operator,
    /// We're looking for a threshold
    Threshold,
    /// Unknown state
    Open,
}

#[derive(PartialEq, Eq, Debug)]
enum ParseError {
    NoPointSpecifier(String),
    NoSeriesSpecifier(String),
    InvalidOperator(String),
    InvalidThreshold(String),
    NoRatioSpecifier(String),
    NoStatusSpecifier(String),
    SyntaxError(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ParseError::*;
        let msg = match *self {
            NoPointSpecifier(ref msg)
            | NoSeriesSpecifier(ref msg)
            | InvalidOperator(ref msg)
            | InvalidThreshold(ref msg)
            | NoRatioSpecifier(ref msg)
            | NoStatusSpecifier(ref msg)
            | SyntaxError(ref msg) => msg,
        };
        write!(f, "{}", msg)
    }
}

#[derive(Debug, PartialEq)]
enum PointAssertion {
    Ratio(f64),
    Recent(usize),
}

/// convert "all" -> 1, "at least 70% (points|series)" -> 0.7
fn parse_ratio<'a, 'b, I>(it: &'b mut I, word: &str) -> Result<PointAssertion, ParseError>
where
    I: Iterator<Item = &'a str>,
{
    use PointAssertion::*;
    let ratio;

    // chew through
    //   "any"
    //   "at least NN% of"

    if word == "any" {
        ratio = Ok(Ratio(0.0));
    } else if word == "all" {
        ratio = Ok(Ratio(1.0))
    } else if word == "at" {
        let mut rat = None;
        while let Some(word) = it.next() {
            if word == "least" {
                // 'at least'
            } else if word.find('%') == Some(word.len() - 1) {
                rat = word[..word.len() - 1].parse::<f64>().ok();
                break;
            } else if word == "points" || word == "point" {
                return Err(ParseError::NoPointSpecifier(format!(
                    "Expected ratio specifier \
                     before '{}'",
                    word
                )));
            } else if word == "series" {
                return Err(ParseError::NoSeriesSpecifier(format!(
                    "Expected ratio specifier \
                     before '{}'",
                    word
                )));
            } else {
                return Err(ParseError::NoRatioSpecifier(format!(
                    "This shouldn't happen: {}, \
                     word.find('%'): {:?}, len: {}",
                    word,
                    word.find('%'),
                    word.len()
                )));
            }
        }
        ratio = Ok(Ratio(rat.expect("Couldn't find ratio for blah") / 100f64))
    } else if word == "most" {
        match it.next() {
            Some(word) if word == "recent" => {
                // yay
            }
            Some(word) => {
                return Err(ParseError::SyntaxError(format!(
                    "Expected 'most recent' \
                     found 'most {}'",
                    word
                )))
            }
            None => {
                return Err(ParseError::SyntaxError(
                    "Expected 'most recent' found trailing 'most'".to_owned(),
                ))
            }
        }
        match it.next() {
            Some(word) if word == "point" => return Ok(Recent(1)),
            Some(word) => {
                return Err(ParseError::SyntaxError(format!(
                    "Expected 'most recent point' found \
                     'most recent {}'",
                    word
                )))
            }
            None => {
                return Err(ParseError::SyntaxError(
                    "Expected 'most recent point' found \
                     trailing 'most recent'"
                        .to_owned(),
                ))
            }
        }
    } else {
        ratio = Err(ParseError::SyntaxError(format!(
            "Expected 'any', 'all', 'most' or 'at \
             least', found '{}'",
            word
        )))
    }

    if ratio.is_ok() {
        // chew stop words
        for word in it {
            // chew through terminators
            if word == "of" {
                continue;
            } else if word == "points" || word == "point" || word == "series" {
                break;
            } else {
                return Err(ParseError::SyntaxError(
                    format!("Expected 'of points|series', found '{}'", word),
                ));
            }
        }
    }

    ratio
}

// Whether or not the operator in the assertion is negated
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NegOp {
    // For situations like `are not`
    Yes,
    // Just `are`
    No,
}

fn parse_assertion(assertion: &str) -> Result<Assertion, ParseError> {
    let mut state = AssertionState::Status;
    let mut operator: Option<&str> = None;
    let mut threshold: Option<f64> = None;
    let mut status = None;
    let mut point_assertion = None;
    let mut series_ratio = 0.0;
    let mut it = assertion.split(' ').peekable();
    let mut negated: NegOp = NegOp::No;

    while let Some(word) = it.next() {
        match state {
            AssertionState::Status => {
                status = match word {
                    "critical" => Some(Status::Critical),
                    "warning" => Some(Status::Warning),
                    _ => {
                        return Err(ParseError::NoStatusSpecifier(format!(
                            "Expect assertion to start with 'critical' \
                             or 'warning', not '{}'",
                            word
                        )))
                    }
                };
                if let Some(next) = it.next() {
                    if next != "if" {
                        return Err(ParseError::SyntaxError(format!(
                            "Expected 'if' to follow '{}', found '{}'",
                            word,
                            next
                        )));
                    }
                } else {
                    return Err(ParseError::SyntaxError(
                        format!("Unexpected end of input after '{}'", word),
                    ));
                }
                state = AssertionState::Points;
            }
            AssertionState::Points => {
                point_assertion = Some(try!(parse_ratio(&mut it, word)));
                state = AssertionState::Open;
            }
            AssertionState::Open => if word == "in" {
                state = AssertionState::Series
            } else if word == "is" || word == "are" {
                if it.peek() == Some(&"not") {
                    negated = NegOp::Yes;
                    it.next();
                }
                state = AssertionState::Operator
            } else {
                return Err(ParseError::SyntaxError(format!(
                    "Expected 'in' or 'is'/'are' \
                     (series spec or operator), \
                     found '{}'",
                    word
                )));
            },
            AssertionState::Series => {
                if let PointAssertion::Ratio(r) = try!(parse_ratio(&mut it, word)) {
                    series_ratio = r;
                } else {
                    return Err(ParseError::SyntaxError(
                        "You can't specify a most recent series, \
                         it doesn't make sense."
                            .to_owned(),
                    ));
                }
                state = AssertionState::Open;
            }
            AssertionState::Operator => if word == "be" {
            } else {
                if let Some(word) = ["<", "<=", ">", ">=", "==", "!="]
                    .iter()
                    .find(|&&op| op == word)
                {
                    operator = Some(word);
                    state = AssertionState::Threshold;
                } else {
                    return Err(ParseError::InvalidOperator(format!(
                        "Expected a comparison \
                         operator (e.g. >=), \
                         not '{}'",
                        word.to_owned()
                    )));
                }
            },
            AssertionState::Threshold => if let Ok(thresh) = word.parse::<f64>() {
                threshold = Some(thresh)
            } else {
                return Err(ParseError::InvalidThreshold(format!(
                    "Couldn't parse float from \
                     '{}'",
                    word
                )));
            },
        }
    }

    if threshold.is_none() {
        return Err(ParseError::InvalidThreshold(format!(
            "No threshold found (e.g. '{0} N', not \
             '{0}')",
            operator.unwrap_or(">=")
        )));
    }

    Ok(Assertion {
        operator: operator.expect("No operator found in predicate").to_owned(),
        op_is_negated: negated,
        threshold: threshold.expect("No threshold found in predicate"),
        point_assertion: point_assertion.expect("No point ratio found in predicate"),
        series_ratio: series_ratio,
        failure_status: status.expect("Needed to start with an exit status"),
    })
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args = parse_args();
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
    use chrono::naive::NaiveDateTime;
    use serde_json;

    use tabin_plugins::Status;

    use super::{do_check, filter_to_with_data, operator_string_to_func, parse_assertion,
                Assertion, DataPoint, GraphiteData, NegOp, ParseError, ASSERTION_EXAMPLES};
    use super::PointAssertion::*;

    #[test]
    fn all_examples_are_accurate() {
        for assertion in ASSERTION_EXAMPLES {
            println!("testing `{}`", assertion);
            parse_assertion(assertion).unwrap();
        }
    }

    fn json_two_sets_of_graphite_data() -> &'static str {
        r#"
        [
            {
                "datapoints": [[null, 11110], [null, 11130]],
                "target": "test.path.no-data"
            },
            {
                "datapoints": [[1, 11150], [null, 11160], [3, 11170]],
                "target": "test.path.some-data"
            }
        ]
        "#
    }

    fn valid_data_from_json_two_sets() -> Vec<GraphiteData> {
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

    #[test]
    fn graphite_result_to_vec_creates_a_vec_of_GraphiteData() {
        let vec: Vec<GraphiteData> = deser(json_two_sets_of_graphite_data());
        assert_eq!(vec.len(), 2)
    }

    fn dt(t: i64) -> NaiveDateTime {
        NaiveDateTime::from_timestamp(t, 0)
    }

    #[test]
    fn operator_string_to_func_returns_a_good_filter() {
        let invalid = operator_string_to_func("<", NegOp::Yes, 5_f64);
        assert!(invalid(6_f64))
    }

    #[test]
    fn filtered_to_with_data_returns_valid_data() {
        let result = filter_to_with_data(
            "test.path",
            deser(json_two_sets_of_graphite_data()),
            Status::Unknown,
        );
        let expected = valid_data_from_json_two_sets();
        match result {
            Ok(actual) => assert_eq!(actual, expected),
            Err(_) => panic!("wha"),
        }
    }

    #[test]
    fn do_check_errors_with_invalid_data() {
        let result = do_check(
            &valid_data_from_json_two_sets(),
            ">",
            NegOp::Yes,
            2.0,
            Ratio(0.0),
            Status::Critical,
        );
        if let Status::Critical = result {
            // expected
        } else {
            panic!("Expected an Status::Critical, got: {:?}", result)
        }
    }

    #[test]
    fn do_check_succeeds_with_valid_data() {
        let result = do_check(
            &valid_data_from_json_two_sets(),
            ">",
            NegOp::Yes,
            0.0,
            Ratio(1.0),
            Status::Critical,
        );
        if let Status::Ok = result {
            // expected
        } else {
            panic!("Expected an Status::Ok, got: {:?}", result)
        }
    }

    #[test]
    fn parse_assertion_requires_a_starting_status() {
        let result = parse_assertion("any point is not < 100");
        if let &Err(ref e) = &result {
            if let &ParseError::NoStatusSpecifier(_) = e {
                // expected
            } else {
                panic!("Unexpected result: {:?}", result)
            }
        } else {
            panic!("Unexpected success: {:?}", result)
        }
    }

    #[test]
    fn parse_assertion_finds_per_point_description() {
        let predicates = parse_assertion("critical if any point is not < 100").unwrap();

        assert_eq!(predicates.operator, "<");
        assert_eq!(predicates.threshold, 100_f64);
        assert_eq!(predicates.point_assertion, Ratio(0.0));
    }

    #[test]
    fn parse_assertion_finds_per_point_description2() {
        let predicates = parse_assertion("critical if any point is not >= 5.5").unwrap();

        assert_eq!(predicates.operator, ">=");
        assert_eq!(predicates.threshold, 5.5_f64);
        assert_eq!(predicates.point_assertion, Ratio(0.));
    }

    fn json_one_point_is_below_5_5() -> &'static str {
        r#"
            [
                {
                    "datapoints": [[6, 60], [7, 70], [8, 80]],
                    "target": "test.path.has-data"
                },
                {
                    "datapoints": [[5, 50], [6, 60]],
                    "target": "test.path.has-data"
                }
            ]
        "#
    }

    fn deser(s: &str) -> Vec<GraphiteData> {
        let result = serde_json::from_str(s);
        result.unwrap()
    }

    #[test]
    fn parse_assertion_finds_per_point_description2_and_correctly_alerts() {
        let assertion = parse_assertion("critical if any point is not >= 5.5").unwrap();
        let graphite_data = deser(json_one_point_is_below_5_5());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        if let Status::Critical = result {
            // expected
        } else {
            panic!("Expected Critical status, not '{:?}'", result)
        }
    }

    #[test]
    fn parse_series() {
        let assertion =
            parse_assertion("critical if any point in any series is not >= 5.5").unwrap();
        assert_eq!(assertion.point_assertion, Ratio(0.0));
        assert_eq!(assertion.series_ratio, 0.0);
    }

    #[test]
    fn parse_some_series() {
        let assertion = parse_assertion(
            "critical if any point in at least 20% of series is not \
             >= 5.5",
        ).unwrap();
        assert_eq!(assertion.point_assertion, Ratio(0.0));
        assert_eq!(assertion.series_ratio, 0.2_f64);
    }

    fn json_all_points_above_5() -> &'static str {
        r#"
            [
                {
                    "datapoints": [[6, 60], [7, 70], [8, 80], [9, 90]],
                    "target": "test.path.has-data"
                }
            ]
        "#
    }

    #[test]
    fn parse_all_points_and_critical() {
        let assertion = parse_assertion("critical if all points are > 5").unwrap();
        assert_eq!(assertion.point_assertion, Ratio(1.0));

        let graphite_data = deser(json_all_points_above_5());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_all_points_and_ok() {
        let assertion = parse_assertion("critical if all points are > 5").unwrap();
        assert_eq!(assertion.point_assertion, Ratio(1.0));

        let graphite_data = deser(json_80p_of_points_are_below_6());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Ok);
    }

    #[test]
    fn parse_most_recent_point() {
        let assertion = parse_assertion("critical if most recent point is > 5").unwrap();
        assert_eq!(
            assertion,
            Assertion {
                operator: ">".into(),
                op_is_negated: NegOp::No,
                threshold: 5.0,
                point_assertion: Recent(1),
                series_ratio: 0.0,
                failure_status: Status::Critical,
            }
        )
    }

    fn json_last_point_is_5() -> &'static str {
        r#"
            [
                {
                    "datapoints": [[2, 20], [3, 30], [4, 40], [5, 50]],
                    "target": "test.path.has-data"
                }
            ]
        "#
    }

    fn json_last_existing_point_is_5() -> &'static str {
        r#"
            [
                {
                    "datapoints": [[4, 40], [5, 50], [null, 60], [null, 70]],
                    "target": "test.path.has-data"
                }
            ]
        "#
    }

    #[test]
    fn most_recent_is_non_empty_works() {
        let assertion = parse_assertion("critical if most recent point is > 5").unwrap();
        let graphite_data = deser(&json_last_point_is_5());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Ok);

        let assertion = parse_assertion("critical if most recent point is > 4").unwrap();
        let graphite_data = deser(json_last_point_is_5());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn most_recent_is_empty_works() {
        let assertion = parse_assertion("critical if most recent point is > 5").unwrap();
        let graphite_data = deser(json_last_existing_point_is_5());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Ok);

        let assertion = parse_assertion("critical if most recent point is > 4").unwrap();
        let graphite_data = deser(json_last_existing_point_is_5());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn most_recent_finds_okay_values_after_invalid() {
        let assertion = parse_assertion("critical if most recent point is == 4").unwrap();
        let graphite_data = deser(json_last_point_is_5());

        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Ok);
    }

    fn json_80p_of_points_are_below_6() -> &'static str {
        r#"
            [
                {
                    "datapoints": [[2, 20], [3, 30], [4, 40], [5, 50], [6, 60]],
                    "target": "test.path.has-data"
                }
            ]
        "#
    }

    #[test]
    fn parse_some_series_and_correctly_alerts() {
        let assertion =
            parse_assertion("critical if at least 80% of of points are not >= 5.5").unwrap();
        let graphite_data = deser(json_80p_of_points_are_below_6());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_some_series_positive_assertion_and_correctly_allows() {
        let assertion = parse_assertion("critical if at least 79% of of points are < 6").unwrap();
        let graphite_data = deser(json_80p_of_points_are_below_6());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_some_series_positive_assertion__and_correctly_allows_all_points() {
        let assertion = parse_assertion("critical if at least 79% of of points are < 6").unwrap();
        let graphite_data = deser(json_80p_of_points_are_below_6());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_some_points() {
        let assertion =
            parse_assertion("critical if at least 20% of points are not >= 5.5").unwrap();
        assert_eq!(assertion.point_assertion, Ratio(0.2));
        assert_eq!(assertion.series_ratio, 0.0);
    }

    #[test]
    fn parse_some_points_and_some_series() {
        let assertion = parse_assertion(
            "critical if at least 80% of points in at least 90% of \
             series are not >= 5.5",
        ).unwrap();
        assert_eq!(assertion.point_assertion, Ratio(0.8));
        assert_eq!(assertion.series_ratio, 0.9_f64);
    }

    fn json_all_points_are_0_and_40p_of_points_are_null() -> &'static str {
        r#"
        [
            {
                "datapoints": [[0, 20], [0, 30], [0, 40], [null, 50], [null, 60]],
                "target": "test.path.has-data"
            }
        ]
        "#
    }

    #[test]
    fn null_points_count_towards_percent() {
        let assertion = parse_assertion("critical if at least 61% of points are == 0").unwrap();
        let graphite_data = deser(json_all_points_are_0_and_40p_of_points_are_null());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Ok);
    }

    #[test]
    fn null_points_count_towards_percent_crit() {
        let assertion = parse_assertion("critical if at least 60% of points are == 0").unwrap();
        let graphite_data = deser(json_all_points_are_0_and_40p_of_points_are_null());
        let result = do_check(
            &graphite_data,
            &assertion.operator,
            assertion.op_is_negated,
            assertion.threshold,
            assertion.point_assertion,
            assertion.failure_status,
        );
        assert_eq!(result, Status::Critical);
    }
}

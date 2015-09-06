#![cfg_attr(test, feature(plugin))]
#![cfg_attr(test, plugin(passert_macros))]

#[macro_use]
extern crate clap;
extern crate chrono;
extern crate hyper;
#[cfg(test)] extern crate passert;
extern crate rustc_serialize;

extern crate turbine_plugins;

use turbine_plugins::Status;

use chrono::naive::datetime::NaiveDateTime;
use rustc_serialize::json::{self, Json};

use std::io::Read;
use std::fmt;

/// One of the datapoints that graphite has returned.
///
/// Graphite always returns all values in its time range, even if it hasn't got
/// any data for them, so the val might not exist.
#[derive(Debug, PartialEq, Clone)]
struct DataPoint {
    val: Option<f64>,
    time: NaiveDateTime
}

impl<'a> From<&'a Json> for DataPoint {
    /// Convert a [value, timestamp] json list into a datapoint
    /// panics if the list is of an invalid format
    fn from(point: &Json) -> DataPoint {
        DataPoint {
            val: match point[0] {
                Json::Null => None,
                Json::F64(n) => Some(n),
                Json::U64(n) => Some(n as f64),
                Json::I64(n) => Some(n as f64),
                _ => panic!(format!("Unexpected float type: {:?}", point[0]))
            },
            time: if let Json::U64(n) = point[1] {
                NaiveDateTime::from_timestamp(n as i64, 0)
            } else {
                panic!(format!("Unexpected timestamp type: {:?}", point[1]))
            }
        }
    }
}

impl fmt::Display for DataPoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} (at {})",
               self.val.map(|v| format!("{}", v)).unwrap_or("No Data".into()),
               self.time.format("%H:%MZ"))
    }
}

/// All the data for one fully-resolved target
#[derive(PartialEq, Debug)]
struct GraphiteData {
    points: Vec<DataPoint>,
    target: String
}

/// Represent the data that we received after some filtering operation
#[derive(Debug, PartialEq)]
struct FilteredGraphiteData<'a> {
    original: &'a GraphiteData,
    points: Vec<&'a DataPoint>
}

impl GraphiteData {
    fn from_json_obj (obj: &Json) -> GraphiteData {
        let dl = obj
            .find("datapoints").expect("Could not find datapoints in obj")
            .as_array().expect("Graphite did not return an array").into_iter()
            .map(DataPoint::from)
            .collect();
        GraphiteData {
            points: dl,
            target: obj.find("target").expect("Couldn't find target in graphite data")
                       .as_string().unwrap().to_owned()
        }
    }

    /// The number of points that we received
    fn len(&self) -> usize {
        self.points.len()
    }

    /// Strip out any invalid points
    fn into_only_with_data(mut self) -> Self {
        let new_points = self.points.into_iter()
            .filter(|point| point.val.is_some())
            .collect();
        self.points = new_points;
        self
    }

    /// References to the points that exist and do not satisfy the comparator
    fn invalid_points(&self, comparator: &Box<Fn(f64) -> bool>) -> Vec<&DataPoint> {
        self.points.iter().filter( |p| p.val.map_or(false, |v| comparator(v)) ).collect()
    }
}

impl<'a> FilteredGraphiteData<'a> {
    fn len(&self) -> usize {
        self.points.len()
    }
}

struct GraphiteIterator {
    current: usize,
    back: usize,
    data: GraphiteData
}

impl Iterator for GraphiteIterator {
    type Item = DataPoint;
    fn next(&mut self) -> Option<DataPoint> {
        self.current += 1;
        self.data.points.get(self.current - 1).map(|p| p.clone())
    }
}

impl IntoIterator for GraphiteData {
    type Item = DataPoint;
    type IntoIter = GraphiteIterator;
    fn into_iter(self) -> Self::IntoIter {
        GraphiteIterator {
            current: 0,
            back: self.points.len(),
            data: self
        }
    }
}

impl DoubleEndedIterator for GraphiteIterator {
    fn next_back(&mut self) -> Option<DataPoint> {
        self.back -= 1;
        self.data.points.get(self.back).map(|p| p.clone())
    }
}

fn graphite_result_to_vec(data: &Json) -> Vec<GraphiteData> {
    data.as_array().expect("Graphite should return an array").iter()
        .map(GraphiteData::from_json_obj).collect()
}

#[cfg_attr(test, allow(dead_code))]
fn get_graphite<S: Into<String>, T: Into<String>>(url: S, target: T, window: i64) -> String {
    let full_path = format!("{}/render?target={}&format=json&from=-{}min",
                            url.into(), target.into(), window);
    let c = hyper::Client::new();
    let mut result = c.get(&full_path).send().unwrap();
    let mut s = String::new();
    result.read_to_string(&mut s).unwrap();
    s
}

/// Take an operator and a value and return a function that can be used in a
/// filter to return only values that *do not* satisfy the operator.
///
/// aka a function that returns only invalid numbers
fn operator_string_to_func(op: &str, op_is_negated: NegOp, val: f64) -> Box<Fn(f64) -> bool> {
    let val = val.clone();
    let comp: Box<Fn(f64) -> bool> = match op {
        "<"  => Box::new(move |i: f64| i <  val),
        "<=" => Box::new(move |i: f64| i <= val),
        ">"  => Box::new(move |i: f64| i >  val),
        ">=" => Box::new(move |i: f64| i >= val),
        "==" => Box::new(move |i: f64| i == val),
        "!=" => Box::new(move |i: f64| i != val),
        a => panic!("Bad operator: {}", a)
    };

    if op_is_negated == NegOp::Yes {
        Box::new(move |i: f64| !comp(i))
    } else {
        comp
    }
}

fn filter_to_with_data(data: Json,
                       no_data_status: Status) -> Result<Vec<GraphiteData>, Status> {
    let data = graphite_result_to_vec(&data);
    let series_with_data = data.into_iter()
        .map(|series| series.into_only_with_data())
        .filter(|series| !series.points.is_empty())
        .collect::<Vec<GraphiteData>>();

    if !series_with_data.is_empty() {
        Ok(series_with_data)
    } else {
        println!("{}: Graphite returned no matches", no_data_status);
        Err(no_data_status)
    }
}

#[allow(deprecated)] // connect => join in 1.3
fn do_check(
    series_with_data: Vec<GraphiteData>,
    op: &str,
    op_is_negated: NegOp,
    threshold: f64,
    error_condition: PointAssertion
) -> Status {
    let comparator = operator_string_to_func(op, op_is_negated, threshold);
    // We want to create a vec of series' that only have (existing) invalid
    // points. The first element is the original length of the vector of points
    let with_invalid = match error_condition {
        // Here, invalid points can exist anywhere
        PointAssertion::Ratio(error_ratio) => series_with_data.iter()
            .map(|series| FilteredGraphiteData {
                original: &series,
                points: series.invalid_points(&comparator)
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
        PointAssertion::Recent(count) => series_with_data.iter()
            .map(|ref series| FilteredGraphiteData {
                original: &series,
                points: series
                    .invalid_points(&comparator)
                    .into_iter().rev().take(count)
                    .collect::<Vec<&DataPoint>>()
            })
            .filter(|ref invalid| invalid.points.len() > 0)
            .collect::<Vec<(FilteredGraphiteData)>>()
    };

    let nostr = if op_is_negated == NegOp::Yes { " not" } else { "" };
    if with_invalid.len() > 0 {
        match error_condition {
            PointAssertion::Ratio(count) => {
                println!("CRITICAL: Of {} paths with data, {} have at least {:.1}% invalid datapoints:",
                         series_with_data.len(), with_invalid.len(), count * 100.0);
                for series in with_invalid.iter() {
                    println!(
                        "       -> {} has {} ({:.1}%) points that are{} {} {}: {}",
                        series.original.target, series.points.len(),
                        (series.len() as f64 / series.original.len() as f64) * 100.0,
                        nostr, op, threshold,
                        series.points.iter().map(|gv| format!("{}", gv))
                            .collect::<Vec<_>>().connect(", "));
                }
            },
            PointAssertion::Recent(count) => {
                println!("CRITICAL: Of {} paths with data, {} have the last {} points invalid",
                         series_with_data.len(), with_invalid.len(), count);
                for series in with_invalid.iter() {
                    println!(
                        "       -> {}'s last {} points are{} {} {}: {}",
                        series.original.target, count, nostr, op, threshold,
                        series.points.iter().map(|gv| format!("{}", gv))
                            .collect::<Vec<_>>().connect(", "));
                }
            }
        }
        Status::Critical
    } else {
        match error_condition {
            PointAssertion::Ratio(percent) => {
                println!(
                    "OK: Found {} paths with data, none had at least {:.1}% of datapoints{} {} {}.",
                    series_with_data.len(), percent * 100.0, nostr, op, threshold);
            },
            PointAssertion::Recent(count) => {
                println!(
                    "OK: Found {} paths with data, none had their last {} of datapoints{} {} {}.",
                    series_with_data.len(), count, nostr, op, threshold
                    );
            }
        };
        for series in series_with_data.iter() {
            println!("    -> {}'s looks like: {}",
                     series.target,
                     series.points.iter()
                     .map(|gv| format!("{}", gv))
                     .collect::<Vec<_>>().connect(", "))
        }
        Status::Ok
    }
}

struct Args {
    url: String,
    path: String,
    assertion: Assertion,
    window: i64,
    no_data: Status
}

static ASSERTION_EXAMPLES: &'static [&'static str] = &[
    "critical if any point is > 0",
    "warning if any point is == 9",
    "critical if all points are > 100.0",
    "critical if at least 20% of points are > 100",
    "critical if most recent point is > 5",
    "critical if most recent point in all series are == 0",
    ];

#[allow(deprecated)]  // connect ->
fn parse_args<'a>() -> Args {
    let allowed_no_data = Status::str_values(); // block-local var for borrowck
    let args = clap::App::new("check-graphite")
        .version("0.1.0")
        .author("Brandon W Maister <quodlibetor@gmail.com>")
        .about("Query graphite and exit based on predicates")
        .args_from_usage(
            "<URL>                'The domain to query graphite. Must include scheme (http/s)'
             <PATH>               'The graphite path to query. For example: \"collectd.*.cpu\"'
             <ASSERTION>          'The assertion to make against the PATH. See Below.'
             -w --window=[WINDOW] 'How many minutes of data to test. Default 10.'")
        .arg(clap::Arg::with_name("NO_DATA_STATUS")
                       .long("--no-data")
                       .help("What to do with no data. Choices: ok, warn, critical, unknown. Default: warn.")
                       .takes_value(true)
                       .possible_values(&allowed_no_data)
             )
        .after_help(&format!("About Assertions:

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

        - `{}`", ASSERTION_EXAMPLES.connect("`\n        - `")))
     .get_matches();

    let assertion_str = args.value_of("ASSERTION").unwrap();
    Args {
        url: args.value_of("URL").unwrap().to_owned(),
        path: args.value_of("PATH").unwrap().to_owned(),
        assertion: parse_assertion(assertion_str).unwrap(),
        window: value_t!(args.value_of("WINDOW"), i64).unwrap_or(10),
        no_data: Status::from_str(args.value_of("NO_DATA_STATUS")
                                      .unwrap_or("warning")).unwrap()
    }
}

#[derive(Debug, PartialEq)]
struct Assertion {
    operator: String,
    op_is_negated: NegOp,
    threshold: f64,
    point_assertion: PointAssertion,
    series_ratio: f64,
    failure_status: Status
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
    Open
}

#[derive(Debug)]
enum ParseError {
    NoPointSpecifier(String),
    NoSeriesSpecifier(String),
    InvalidOperator(String),
    InvalidThreshold(String),
    NoRatioSpecifier(String),
    NoStatusSpecifier(String),
    SyntaxError(String)
}

#[derive(Debug, PartialEq)]
enum PointAssertion {
    Ratio(f64),
    Recent(usize)
}

/// convert "all" -> 1, "at least 70% (points|series)" -> 0.7
fn parse_ratio<'a, 'b, I>(it: &'b mut I, word: &str) -> Result<PointAssertion, ParseError>
    where I: Iterator<Item=&'a str>
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
            if word == "least" { /* 'at least' */ }
            else if word.find('%') == Some(word.len() - 1) {
                rat = word[..word.len() - 1].parse::<f64>().ok();
                break;
            } else if word == "points" || word == "point" {
                return Err(ParseError::NoPointSpecifier(
                    format!("Expected ratio specifier before '{}'", word)));
            } else if word == "series" {
                return Err(ParseError::NoSeriesSpecifier(
                    format!("Expected ratio specifier before '{}'", word)));
            } else {
                return Err(ParseError::NoRatioSpecifier(
                    format!("This shouldn't happen: {}, word.find('%'): {:?}, len: {}",
                            word, word.find('%'), word.len())));
            }
        }
        ratio = Ok(Ratio(rat.expect("Couldn't find ratio for blah") / 100f64))
    } else if word == "most" {
        match it.next() {
            Some(word) if word == "recent" => { /* yay */ },
            Some(word) => return Err(ParseError::SyntaxError(
                format!("Expected 'most recent' found 'most {}'",
                        word))),
            None => return Err(ParseError::SyntaxError(
                format!("Expected 'most recent' found trailing 'most'")))
        };
        match it.next() {
            Some(word) if word == "point" => return Ok(Recent(1)),
            Some(word) => return Err(ParseError::SyntaxError(
                format!("Expected 'most recent point' found 'most recent {}'",
                        word))),
            None => return Err(ParseError::SyntaxError(
                format!("Expected 'most recent point' found trailing 'most recent'")))
        }
    } else {
        ratio = Err(ParseError::SyntaxError(
            format!("Expected 'any', 'all', 'most' or 'at least', found '{}'", word)))
    }

    if ratio.is_ok() {
        // chew stop words
        while let Some(word) = it.next() {
            // chew through terminators
            if word == "of" { continue; }
            else if word == "points" || word == "point" { break; }
            else if word == "series" { break; }
            else {
                return Err(ParseError::SyntaxError(
                    format!("Expected 'of points|series', found '{}'", word)))
            }
        }
    }

    return ratio;
}

// Whether or not the operator in the assertion is negated
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NegOp {
    // For situations like `are not`
    Yes,
    // Just `are`
    No
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
                    _ => return Err(ParseError::NoStatusSpecifier(format!(
                        "Expect assertion to start with 'critical' or 'warning', not '{}'", word)))
                };
                if let Some(next) = it.next() {
                    if next != "if" {
                        return Err(ParseError::SyntaxError(format!(
                                "Expected 'if' to follow '{}', not '{}'", word, next)));
                    }
                } else {
                    return Err(ParseError::SyntaxError(format!(
                                "Unexpected end of input after '{}'", word)));
                }
                state = AssertionState::Points;
            },
            AssertionState::Points => {
                point_assertion = Some(try!(parse_ratio(&mut it, word)));
                state = AssertionState::Open;
            },
            AssertionState::Open => {
                if word == "in" {
                    state = AssertionState::Series
                } else if word == "is" || word == "are" {
                    if it.peek() == Some(&"not") {
                        negated = NegOp::Yes;
                        it.next();
                    }
                    state = AssertionState::Operator
                } else {
                    return Err(ParseError::SyntaxError(
                        format!("Expected 'in' or 'is not' or 'are not', found '{}'", word)))
                }
            },
            AssertionState::Series => {
                if let PointAssertion::Ratio(r) = try!(parse_ratio(&mut it, word)) {
                    series_ratio = r;
                } else {
                    return Err(ParseError::SyntaxError(
                        "You can't specify a most recent series, \
                         it doesn't make sense.".to_string()));
                }
                state = AssertionState::Open;
            },
            AssertionState::Operator => {
                if word == "be" {}
                else {
                    if let Some(word) = ["<", "<=", ">", ">=", "==", "!="].iter()
                                        .find(|&&op| op == word) {
                        operator = Some(word);
                        state = AssertionState::Threshold;
                    } else {
                        return Err(ParseError::InvalidOperator(word.to_owned()))
                    }
                }
            },
            AssertionState::Threshold => {
                if let Ok(thresh) = word.parse::<f64>() {
                    threshold = Some(thresh)
                } else {
                    return Err(ParseError::InvalidThreshold(format!(
                        "Couldn't parse float from '{}'", word)))
                }
            }
        }
    }

    Ok(Assertion {
        operator: operator.expect("No operator found in predicate").to_owned(),
        op_is_negated: negated,
        threshold: threshold.expect("No threshold found in predicate"),
        point_assertion: point_assertion.expect("No point ratio found in predicate"),
        series_ratio: series_ratio,
        failure_status: status.expect("Needed to start with an exit status")
    })
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args = parse_args();
    let json_str = get_graphite(args.url, args.path, args.window);

    let data = json::Json::from_str(&json_str).unwrap();

    let filtered = filter_to_with_data(data, args.no_data);
    let with_data = match filtered {
        Ok(data) => data,
        Err(status) => status.exit()
    };

    let status = do_check(with_data,
                          &args.assertion.operator,
                          args.assertion.op_is_negated,
                          args.assertion.threshold,
                          args.assertion.point_assertion);
    status.exit();
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use chrono::naive::datetime::NaiveDateTime;
    use rustc_serialize::json::Json;

    use turbine_plugins::Status;

    use super::{Assertion, GraphiteData, DataPoint, operator_string_to_func,
                graphite_result_to_vec, do_check, ParseError, NegOp,
                filter_to_with_data, parse_assertion, ASSERTION_EXAMPLES};
    use super::PointAssertion::*;

    #[test]
    fn all_examples_are_accurate() {
        for assertion in ASSERTION_EXAMPLES {
            println!("testing `{}`", assertion);
            parse_assertion(assertion).unwrap();
        }
    }

    fn json_two_sets_of_graphite_data() -> Json {
        Json::from_str(r#"
        [
            {
                "datapoints": [[null, 11110], [null, 11130]],
                "target": "test.path.no-data"
            },
            {
                "datapoints": [[1, 11150], [null, 11160], [3, 11160]],
                "target": "test.path.some-data"
            }
        ]
        "#).unwrap()
    }

    fn valid_data_from_json_two_sets() -> Vec<GraphiteData> {
        vec![GraphiteData {
            points: vec![DataPoint { val: Some(1_f64),
                                     time: dt(11150) },
                         DataPoint { val: Some(3_f64),
                                     time: dt(11160) }],
            target: "test.path.some-data".to_owned() }]
    }

    #[test]
    fn graphite_result_to_vec_creates_a_vec_of_GraphiteData() {
        let vec = graphite_result_to_vec(&json_two_sets_of_graphite_data());
        assert_eq!(vec.len(), 2)
    }

    fn dt(t: i64) -> NaiveDateTime { NaiveDateTime::from_timestamp(t, 0) }

    #[test]
    fn operator_string_to_func_returns_a_good_filter() {
        let invalid = operator_string_to_func("<", NegOp::Yes, 5_f64);
        assert!(invalid(6_f64))
    }

    #[test]
    fn filtered_to_with_data_returns_valid_data() {
        let result = filter_to_with_data(json_two_sets_of_graphite_data(),
                                         Status::Unknown);
        let expected = valid_data_from_json_two_sets();
        match result {
            Ok(actual) => passert!(actual == expected),
            Err(_) => panic!("wha")
        }
    }

    #[test]
    fn do_check_errors_with_invalid_data() {
        let result = do_check(valid_data_from_json_two_sets(),
                              ">",
                              NegOp::Yes,
                              2.0,
                              Ratio(0.0));
        if let Status::Critical = result {
             /* expected */
        } else {
            panic!("Expected an Status::Critical, got: {:?}", result)
        }
    }

    #[test]
    fn do_check_succeeds_with_valid_data() {
        let result = do_check(valid_data_from_json_two_sets(),
                              ">",
                              NegOp::Yes,
                              0.0,
                              Ratio(1.0));
        if let Status::Ok = result {
            /* expected */
        } else {
            panic!("Expected an Status::Ok, got: {:?}", result)
        }
    }

    #[test]
    fn parse_assertion_requires_a_starting_status() {
        let result = parse_assertion("any point is not < 100");
        if let &Err(ref e) = &result {
            if let &ParseError::NoStatusSpecifier(_) = e {
                /* expected */
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

    fn json_one_point_is_below_5_5() -> Json {
        Json::from_str(r#"
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
        "#).unwrap()
    }

    #[test]
    fn parse_assertion_finds_per_point_description2_and_correctly_alerts() {
        let assertion = parse_assertion("critical if any point is not >= 5.5").unwrap();
        let graphite_data = graphite_result_to_vec(&json_one_point_is_below_5_5());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        if let Status::Critical = result  {
             /* expected */
        } else {
            panic!("Expected Critical status, not '{:?}'", result)
        }
    }

    #[test]
    fn parse_series() {
        let assertion = parse_assertion("critical if any point in any series is not >= 5.5")
            .unwrap();
        assert_eq!(assertion.point_assertion, Ratio(0.0));
        assert_eq!(assertion.series_ratio, 0.0);
    }

    #[test]
    fn parse_some_series() {
        let assertion = parse_assertion(
            "critical if any point in at least 20% of series is not >= 5.5")
            .unwrap();
        assert_eq!(assertion.point_assertion, Ratio(0.0));
        assert_eq!(assertion.series_ratio, 0.2_f64);
    }

    fn json_all_points_above_5() -> Json {
        Json::from_str(r#"
            [
                {
                    "datapoints": [[6, 60], [7, 70], [8, 80], [9, 90]],
                    "target": "test.path.has-data"
                }
            ]
        "#).unwrap()
    }

    #[test]
    fn parse_all_points_and_critical() {
        let assertion = parse_assertion(
            "critical if all points are > 5")
            .unwrap();
        assert_eq!(assertion.point_assertion, Ratio(1.0));

        let graphite_data = graphite_result_to_vec(&json_all_points_above_5());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_all_points_and_ok() {
        let assertion = parse_assertion(
            "critical if all points are > 5")
            .unwrap();
        assert_eq!(assertion.point_assertion, Ratio(1.0));

        let graphite_data = graphite_result_to_vec(&json_80p_of_points_are_below_6());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        assert_eq!(result, Status::Ok);
    }

    #[test]
    fn parse_most_recent_point() {
        let assertion = parse_assertion(
            "critical if most recent point is > 5"
            ).unwrap();
        assert_eq!(assertion,
                   Assertion {
                       operator: ">".into(),
                       op_is_negated: NegOp::No,
                       threshold: 5.0,
                       point_assertion: Recent(1),
                       series_ratio: 0.0,
                       failure_status: Status::Critical
                   })
    }

    fn json_last_point_is_5() -> Json {
        Json::from_str(r#"
            [
                {
                    "datapoints": [[2, 20], [3, 30], [4, 40], [5, 50]],
                    "target": "test.path.has-data"
                }
            ]
        "#).unwrap()
    }

    fn json_last_existing_point_is_5() -> Json {
        Json::from_str(r#"
            [
                {
                    "datapoints": [[4, 40], [5, 50], [null, 60], [null, 70]],
                    "target": "test.path.has-data"
                }
            ]
        "#).unwrap()
    }

    #[test]
    fn most_recent_is_non_empty_works() {
        let assertion = parse_assertion("critical if most recent point is > 5").unwrap();
        let graphite_data = graphite_result_to_vec(&json_last_point_is_5());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        assert_eq!(result, Status::Ok);

        let assertion = parse_assertion("critical if most recent point is > 4").unwrap();
        let graphite_data = graphite_result_to_vec(&json_last_point_is_5());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn most_recent_is_empty_works() {
        let assertion = parse_assertion("critical if most recent point is > 5").unwrap();
        let graphite_data = graphite_result_to_vec(&json_last_existing_point_is_5());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        assert_eq!(result, Status::Ok);

        let assertion = parse_assertion("critical if most recent point is > 4").unwrap();
        let graphite_data = graphite_result_to_vec(&json_last_existing_point_is_5());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        assert_eq!(result, Status::Critical);
    }

    fn json_80p_of_points_are_below_6() -> Json {
        Json::from_str(r#"
            [
                {
                    "datapoints": [[2, 20], [3, 30], [4, 40], [5, 50], [6, 60]],
                    "target": "test.path.has-data"
                }
            ]
        "#).unwrap()
    }

    #[test]
    fn parse_some_series_and_correctly_alerts() {
        let assertion = parse_assertion(
            "critical if at least 80% of of points are not >= 5.5")
            .unwrap();
        let graphite_data = graphite_result_to_vec(&json_80p_of_points_are_below_6());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_some_series_positive_assertion_and_correctly_allows() {
        let assertion = parse_assertion(
            "critical if at least 79% of of points are < 6")
            .unwrap();
        let graphite_data = graphite_result_to_vec(&json_80p_of_points_are_below_6());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_some_series_positive_assertion__and_correctly_allows_all_points() {
        let assertion = parse_assertion(
            "critical if at least 79% of of points are < 6")
            .unwrap();
        let graphite_data = graphite_result_to_vec(&json_80p_of_points_are_below_6());
        let result = do_check(graphite_data,
                              &assertion.operator,
                              assertion.op_is_negated,
                              assertion.threshold,
                              assertion.point_assertion);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_some_points() {
        let assertion = parse_assertion(
            "critical if at least 20% of points are not >= 5.5")
            .unwrap();
        assert_eq!(assertion.point_assertion, Ratio(0.2));
        assert_eq!(assertion.series_ratio, 0.0);
    }

    #[test]
    fn parse_some_points_and_some_series() {
        let assertion = parse_assertion(
            "critical if at least 80% of points in at least 90% of series are not >= 5.5")
            .unwrap();
        assert_eq!(assertion.point_assertion, Ratio(0.8));
        assert_eq!(assertion.series_ratio, 0.9_f64);
    }
}

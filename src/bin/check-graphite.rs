#![cfg_attr(test, feature(plugin))]
#![cfg_attr(test, plugin(clippy))]


#[macro_use]
extern crate clap;
extern crate chrono;
extern crate hyper;
extern crate rustc_serialize;

use chrono::{UTC, Duration};
use chrono::naive::datetime::NaiveDateTime;
use rustc_serialize::json::{self, Json};

use std::io::Read;
use std::fmt;

use std::process;

#[must_use]
#[derive(Debug)]
pub enum ExitStatus {
    Ok,
    Warning,
    Critical,
    Unknown
}

impl ExitStatus {
    #![cfg_attr(test, allow(dead_code))]
    pub fn exit(self) -> ! {
        use self::ExitStatus::*;
        match self {
            Ok => process::exit(0),
            Warning => process::exit(1),
            Critical => process::exit(2),
            Unknown => process::exit(3)
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DataPoint {
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
        write!(f, "{} (at {})", self.val.unwrap(), self.time.format("%H:%MZ"))
    }
}

#[derive(PartialEq, Debug)]
pub struct GraphiteData {
    points: Vec<DataPoint>,
    target: String
}

impl GraphiteData {
    pub fn from_json_obj (obj: &Json) -> GraphiteData {
        let dl = obj
            .find("datapoints").expect("Could not find datapoints in obj")
            .as_array().expect("Graphite did not return an array").into_iter()
            .map(|point| DataPoint::from(point))
            .collect();
        GraphiteData {
            points: dl,
            target: obj.find("target").expect("Couldn't find target in graphite data")
                       .as_string().unwrap().to_owned()
        }
    }

    /// return true if any of our datapoints have actual values
    pub fn has_datapoints(&self) -> bool {
        self.points.iter().all(|point| point.val.is_some())
    }

    /// All data points with values since the time
    pub fn data_since(&self, since: NaiveDateTime) -> Vec<&DataPoint> {
        self.points.iter()
            .filter(|point| point.val.is_some() && point.time >= since)
            .collect()
    }

    /// Strip our points to just include self.data_since points
    /// TODO: this would be much nicer as an &mut self method, but then we'd
    /// need to have a vec of &GraphitePoint's instead of owned
    /// graphitepoints... I think. `into_iter()` doesn't work, anyway.
    pub fn into_only_since(mut self, since: NaiveDateTime) -> Self {
        let new_points = self.points.into_iter()
            .filter(|point| point.val.is_some() && point.time >= since)
            .collect();
        self.points = new_points;
        self
    }

    /// Mutate to only have the invalid points
    /// TODO: this would be much nicer as an &mut self method. See above.
    pub fn into_only_invalid(mut self, comparator: &Box<Fn(f64) -> bool>) -> Self {
        self.points = self.points.into_iter()
                         .filter(|p| comparator(p.val.unwrap()))
                         .collect::<Vec<DataPoint>>();
        self
    }
}

pub struct GraphiteIterator {
    current: usize,
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
            data: self
        }
    }
}

fn graphite_result_to_vec(data: &Json) -> Vec<GraphiteData> {
    data.as_array().expect("Graphite should return an array").iter()
        .map(GraphiteData::from_json_obj).collect()
}

#[cfg_attr(test, allow(dead_code))]
fn get_graphite<S: Into<String>, T: Into<String>>(url: S, target: T) -> String {
    let full_path = format!("{}/render?target={}&format=json", url.into(), target.into());
    let mut c = hyper::Client::new();
    let mut result = c.get(&full_path).send().unwrap();
    let mut s = String::new();
    result.read_to_string(&mut s).unwrap();
    s
}

#[cfg_attr(test, allow(dead_code))]
fn window_to_absolute_time(history_window: i64) -> NaiveDateTime {
    (UTC::now() - Duration::minutes(history_window)).naive_utc()
}

/// Take an operator and a value and return a function that can be used in a
/// filter to return only values that *do not* satisfy the operator.
///
/// aka a function that returns only invalid numbers
#[cfg_attr(test, allow(float_cmp))]
fn operator_string_to_func(op: &str, val: f64) -> Box<Fn(f64) -> bool> {
    let val = val.clone();
    match op {
        "<" => Box::new(move |i: f64| !(i < val)),
        "<=" => Box::new(move |i: f64| !(i <= val)),
        ">" => Box::new(move |i: f64| !(i > val)),
        ">=" => Box::new(move |i: f64| !(i >= val)),
        "==" => Box::new(move |i: f64| !(i == val)),
        "!=" => Box::new(move |i: f64| !(i != val)),
        a => panic!("Bad operator: {}", a)
    }
}

fn filter_to_with_data(data: Json,
                       history_begin: NaiveDateTime,
                       no_data_status: NoDataStatus) -> Result<Vec<GraphiteData>, ExitStatus> {
    let data = graphite_result_to_vec(&data);
    let data_len = data.len();
    let series_with_data = data.into_iter()
        .map(|gd| gd.into_only_since(history_begin))
        .filter(|gd| gd.points.len() > 0)
        .collect::<Vec<GraphiteData>>();

    println!("Found {} matches, but only {} have data", data_len, series_with_data.len());
    if series_with_data.len() > 0 {
        Ok(series_with_data)
    } else {
        match no_data_status {
            NoDataStatus::Ok => Err(ExitStatus::Ok),
            NoDataStatus::Warning => Err(ExitStatus::Warning),
            NoDataStatus::Critical => Err(ExitStatus::Critical),
            NoDataStatus::Unknown => Err(ExitStatus::Unknown)
        }
    }
}

fn do_check(series_with_data: Vec<GraphiteData>, op: &str, threshold: f64) -> ExitStatus {
    let with_data_len = series_with_data.len();
    for graphite_data in series_with_data.iter() {
        let points: Vec<f64> = graphite_data.points.iter().map(|dp| dp.val.unwrap()).collect();
        println!("{} has {} items and looks like: {:?}",
                 graphite_data.target, graphite_data.points.len(), points)
    }

    let comparator = operator_string_to_func(op, threshold);
    let with_invalid_points = series_with_data.into_iter()
        .map(|gd| gd.into_only_invalid(&comparator))
        .filter(|gd| gd.points.len() > 0)
        .collect::<Vec<GraphiteData>>();

    let with_invalid_len = with_invalid_points.len();

    if with_invalid_len > 0 {
        println!("CRITICAL: Of {} paths with data, {} have invalid datapoints:",
                 with_data_len, with_invalid_len);
        for gd in with_invalid_points.iter() {
            println!("{} has {} points that are not {} {}: {}",
                     gd.target,
                     gd.points.len(),
                     op,
                     threshold,
                     gd.points.iter().map(|gv| format!("{}", gv))
                     .collect::<Vec<_>>().connect(", "))
        }
        ExitStatus::Critical
    } else {
        println!("OK: Found {} paths with data, none had invalid datapoints.",
                 with_data_len);
        ExitStatus::Ok
    }
}

enum NoDataStatus {
    Ok,
    Warning,
    Critical,
    Unknown
}

impl NoDataStatus {
    fn from_str(s: &str) -> NoDataStatus {
        use NoDataStatus::*;
        match s {
            "ok" => Ok,
            "warning" => Warning,
            "critical" => Critical,
            "unknown" => Unknown,
            _ => panic!("Unexpected status for no-data: {}", s)
        }
    }

    fn str_values() -> [&'static str; 4] {
        ["ok", "warn", "critical", "unknown"]
    }
}

struct Args {
    url: String,
    path: String,
    operator: String,
    threshold: f64,
    window: i64,
    no_data: NoDataStatus
}

fn parse_args() -> Args {
    let allowed_no_data = NoDataStatus::str_values(); // block-local var for borrowck
    let args = clap::App::new("check-graphite")
        .version("0.1.0")
        .author("Brandon W Maister <quodlibetor@gmail.com>")
        .about("Query graphite and exit based on predicates")
        .args_from_usage(
            "<URL>                'The domain to query graphite.'
             <PATH>               'The graphite path to query.'
             <OPERATOR>           'All values in window must compare OPERATOR to THRESHOLD.'
             <THRESHOLD>          'The value to compare against.'
             -w --window=[WINDOW] 'How many minutes of data to test. Default 10.'")
        .arg(clap::Arg::with_name("NO_DATA_STATUS")
                       .long("--no-data")
                       .help("What to do with no data. Choices: ok, warn, critical, unknown. Default: warn.")
                       .takes_value(true)
                       .possible_values(&allowed_no_data)
             )
        .get_matches();

    Args {
        url: args.value_of("URL").unwrap().to_owned(),
        path: args.value_of("PATH").unwrap().to_owned(),
        operator: args.value_of("OPERATOR").unwrap().to_owned(),
        threshold: value_t!(args.value_of("THRESHOLD"), f64).unwrap(),
        window: value_t!(args.value_of("WINDOW"), i64).unwrap_or(10),
        no_data: NoDataStatus::from_str(args.value_of("NO_DATA_STATUS").unwrap_or("warning"))
    }
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args = parse_args();
    let json_str = get_graphite(args.url, args.path);

    let data = json::Json::from_str(&json_str).unwrap();
    let history_begin = window_to_absolute_time(args.window);

    let filtered = filter_to_with_data(data, history_begin, args.no_data);
    let with_data = match filtered {
        Ok(data) => data,
        Err(status) => status.exit()
    };

    let status = do_check(with_data, &args.operator, args.threshold);
    status.exit();
}

#[cfg(test)]
#[allow(non_snake_case)]
mod test {
    use chrono::naive::datetime::NaiveDateTime;
    use rustc_serialize::json::Json;

    use super::{GraphiteData, DataPoint, operator_string_to_func, graphite_result_to_vec, do_check,
                NoDataStatus, ExitStatus, filter_to_with_data};

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
        "#).ok().expect("Couldn't create test graphite data")
    }

    fn valid_data_from_json_two_sets() -> Vec<GraphiteData> {
        vec![GraphiteData {
            points: vec![DataPoint { val: Some(1_f64),
                                     time: dt(11150) },
                         DataPoint { val: Some(3_f64),
                                     time: dt(11160) }],
            target: "test.path.some-data".to_owned() }]
    }

    fn json_no_data_vals() -> Json {
        Json::from_str(r#"
            {
                "datapoints": [[null, 11110], [null, 11120], [null, 11130]],
                "target": "test.path.no-data"
            }
        "#).ok().expect("Couldn't create test graphite data")
    }

    fn json_two_data_vals_near_the_end() -> Json {
        Json::from_str(r#"
            {
                "datapoints": [[null, 11110], [null, 11120], [null, 11130], [2, 11140],
                               [1, 11150], [null, 11160], [3, 11160]],
                "target": "test.path.some-data"
            }
        "#).ok().expect("Couldn't create test graphite data")
    }

    fn json_some_data_vals() -> Json {
        Json::from_str(r#"
            {
                "datapoints": [[12.4, 11110], [13.4, 11120], [14.4, 11130]],
                "target": "test.path.has-data"
            }
        "#).ok().expect("Couldn't create test graphite data")
    }

    #[test]
    fn graphite_result_to_vec_creates_a_vec_of_GraphiteData() {
        let vec = graphite_result_to_vec(&json_two_sets_of_graphite_data());
        assert_eq!(vec.len(), 2)
    }

    #[test]
    fn has_datapoints_is_false_when_there_is_no_data() {
        let gd = GraphiteData::from_json_obj(&json_no_data_vals());
        assert!(!gd.has_datapoints())
    }

    #[test]
    fn has_datapoints_is_true_when_there_is_data() {
        let gd = GraphiteData::from_json_obj(&json_some_data_vals());
        assert!(gd.has_datapoints())
    }

    #[test]
    fn data_since_returns_data_when_it_exists() {
        let gd = GraphiteData::from_json_obj(&json_two_data_vals_near_the_end());
        fn dt(t: i64) -> NaiveDateTime { NaiveDateTime::from_timestamp(t, 0) }
        let data = gd.data_since(NaiveDateTime::from_timestamp(11140, 0));
        let expected = vec![DataPoint { val: Some(2f64), time: dt(11140) },
                            DataPoint { val: Some(1f64), time: dt(11150) },
                            DataPoint { val: Some(3f64), time: dt(11160) }
                            ];
        assert_eq!(data.len(), 3);
        for (actual, expected) in data.iter().zip(expected) {
            // These are some funky ref/derefs
            assert_eq!(*actual, &expected);
        };
    }

    fn dt(t: i64) -> NaiveDateTime { NaiveDateTime::from_timestamp(t, 0) }

    #[test]
    fn only_since_correctly_strips_down() {
        let gd = GraphiteData::from_json_obj(&json_two_data_vals_near_the_end());
        let gd_stripped = gd.into_only_since(NaiveDateTime::from_timestamp(11140, 0));
        let expected = vec![DataPoint { val: Some(2f64), time: dt(11140) },
                            DataPoint { val: Some(1f64), time: dt(11150) },
                            DataPoint { val: Some(3f64), time: dt(11160) }
                            ];
        assert_eq!(gd_stripped.points.len(), 3);
        for (actual, expected) in gd_stripped.points.iter().zip(expected) {
            // These are some funky ref/derefs
            assert_eq!(*actual, expected);
        };
    }

    #[test]
    fn operator_string_to_func_returns_a_good_filter() {
        let invalid = operator_string_to_func("<", 5_f64);
        assert!(invalid(6_f64))
    }

    #[test]
    fn filtered_to_with_data_returns_valid_data() {
        let result = filter_to_with_data(json_two_sets_of_graphite_data(),
                                         NaiveDateTime::from_timestamp(11140, 0),
                                         NoDataStatus::Unknown);
        let expected = valid_data_from_json_two_sets();
        match result {
            Ok(actual) => assert_eq!(actual,
                                     expected),
            Err(_) => panic!("wha")
        }
    }

    #[test]
    fn do_check_errors_with_invalid_data() {
        let result = do_check(valid_data_from_json_two_sets(),
                              ">",
                              2f64);
        if let ExitStatus::Critical = result {
             /* expected */
        } else {
            panic!("Expected an ExitStatus::Critical, got: {:?}", result)
        }
    }

    #[test]
    fn do_check_succeeds_with_valid_data() {
        let result = do_check(valid_data_from_json_two_sets(),
                              ">",
                              0f64);
        if let ExitStatus::Ok = result {
            /* expected */
        } else {
            panic!("Expected an ExitStatus::Ok, got: {:?}", result)
        }
    }
}

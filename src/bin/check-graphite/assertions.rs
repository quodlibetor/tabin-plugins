use std::fmt;
use std::str::FromStr;

use itertools::Itertools;
use tabin_plugins::Status;

use graphite::{FilteredGraphiteData, GraphiteData};

/// The primary property of
#[derive(Debug, PartialEq)]
pub(crate) struct Assertion {
    pub operator: String,
    pub op_is_negated: NegOp,
    pub threshold: f64,
    pub point_assertion: PointAssertion,
    pub series_ratio: f64,
    pub failure_status: Status,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PointAssertion {
    Ratio(f64),
    Recent(usize),
}

impl Assertion {
    /// Check if any series *violates* the assertion
    ///
    /// Returns the maximum violation status
    ///
    /// `series_with_data` must contain only series that contain at least some
    /// data (`main::bail_if_no_data` must have been called first.)
    pub fn check(&self, series_with_data: &[GraphiteData]) -> Status {
        let &Assertion {
            operator: ref op,
            op_is_negated,
            threshold,
            point_assertion: error_condition,
            failure_status: status,
            ..
        } = self;
        let comparator = operator_string_to_func(op, op_is_negated, threshold);
        // We want to create a vec of series' that only have (existing) invalid
        // points. The first element is the original length of the vector of points
        let with_invalid = match error_condition {
            // Here, invalid points can exist anywhere
            PointAssertion::Ratio(error_ratio) => series_with_data
                .iter()
                .map(|series| FilteredGraphiteData {
                    original: &series,
                    points: series.invalid_points(&comparator),
                })
                .filter(|invalid| {
                    if error_ratio == 0.0 {
                        !invalid.points.is_empty()
                    } else {
                        let filtered = invalid.points.len() as f64;
                        let original = invalid
                            .original
                            .points
                            .iter()
                            .filter(|p| p.val.is_some())
                            .count() as f64;
                        filtered / original >= error_ratio
                    }
                })
                .collect::<Vec<FilteredGraphiteData>>(),
            PointAssertion::Recent(count) => series_with_data
                .iter()
                .map(|ref series| FilteredGraphiteData {
                    original: &series,
                    points: series.last_invalid_points(count, &comparator),
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
                                "{}: All {} matched paths have at least {:.0}% invalid datapoints:",
                                status,
                                with_invalid.len(),
                                ratio * 100.0
                            )
                        }
                    } else {
                        println!(
                            "{}: Of {} paths with data, {} have at least {:.1}% invalid datapoints:",
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
}

impl FromStr for Assertion {
    type Err = ParseError;
    fn from_str(raw: &str) -> Result<Assertion, ParseError> {
        parse_assertion(raw)
    }
}

// Parsing

/// Current state of parsing the assertion
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
pub(crate) enum ParseError {
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
        use self::ParseError::*;
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

// Whether or not the operator in the assertion is negated
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NegOp {
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
                            word, next
                        )));
                    }
                } else {
                    return Err(ParseError::SyntaxError(format!(
                        "Unexpected end of input after '{}'",
                        word
                    )));
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

/// convert "all" -> 1, "at least 70% (points|series)" -> 0.7
fn parse_ratio<'a, 'b, I>(it: &'b mut I, word: &str) -> Result<PointAssertion, ParseError>
where
    I: Iterator<Item = &'a str>,
{
    use self::PointAssertion::*;
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
                return Err(ParseError::SyntaxError(format!(
                    "Expected 'of points|series', found '{}'",
                    word
                )));
            }
        }
    }

    ratio
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

#[cfg(test)]
mod test {
    use tabin_plugins::Status;

    use super::*;
    use super::PointAssertion::*;
    use test::{deser, valid_data_from_json_two_sets};

    #[test]
    fn operator_string_to_func_returns_a_good_filter() {
        let invalid = operator_string_to_func("<", NegOp::Yes, 5_f64);
        assert!(invalid(6_f64))
    }

    #[test]
    fn do_check_errors_with_invalid_data() {
        let result = Assertion {
            operator: ">".into(),
            op_is_negated: NegOp::Yes,
            threshold: 2.0,
            point_assertion: Ratio(0.0),
            series_ratio: 0.0,
            failure_status: Status::Critical,
        }.check(&valid_data_from_json_two_sets());
        if let Status::Critical = result {
            // expected
        } else {
            panic!("Expected an Status::Critical, got: {:?}", result)
        }
    }

    #[test]
    fn do_check_succeeds_with_valid_data() {
        let result = Assertion {
            operator: ">".into(),
            op_is_negated: NegOp::Yes,
            threshold: 0.0,
            point_assertion: Ratio(1.0),
            series_ratio: 0.0,
            failure_status: Status::Critical,
        }.check(&valid_data_from_json_two_sets());
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

    #[test]
    fn parse_assertion_finds_per_point_description2_and_correctly_alerts() {
        let assertion = parse_assertion("critical if any point is not >= 5.5").unwrap();
        let graphite_data = deser(json_one_point_is_below_5_5());
        let result = assertion.check(&graphite_data);
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
        let result = assertion.check(&graphite_data);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_all_points_and_ok() {
        let assertion = parse_assertion("critical if all points are > 5").unwrap();
        assert_eq!(assertion.point_assertion, Ratio(1.0));

        let graphite_data = deser(json_80p_of_points_are_below_6());
        let result = assertion.check(&graphite_data);
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
        let result = assertion.check(&graphite_data);
        assert_eq!(result, Status::Ok);

        let assertion = parse_assertion("critical if most recent point is > 4").unwrap();
        let graphite_data = deser(json_last_point_is_5());
        let result = assertion.check(&graphite_data);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn most_recent_is_empty_works() {
        let assertion = parse_assertion("critical if most recent point is > 5").unwrap();
        let graphite_data = deser(json_last_existing_point_is_5());
        let result = assertion.check(&graphite_data);
        assert_eq!(result, Status::Ok);

        let assertion = parse_assertion("critical if most recent point is > 4").unwrap();
        let graphite_data = deser(json_last_existing_point_is_5());
        let result = assertion.check(&graphite_data);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn most_recent_finds_okay_values_after_invalid() {
        let assertion = parse_assertion("critical if most recent point is == 4").unwrap();
        let graphite_data = deser(json_last_point_is_5());

        let result = assertion.check(&graphite_data);
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
        let result = assertion.check(&graphite_data);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_some_series_positive_assertion_and_correctly_allows() {
        let assertion = parse_assertion("critical if at least 79% of of points are < 6").unwrap();
        let graphite_data = deser(json_80p_of_points_are_below_6());
        let result = assertion.check(&graphite_data);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn parse_some_series_positive_assertion_and_correctly_allows_all_points() {
        let assertion = parse_assertion("critical if at least 79% of of points are < 6").unwrap();
        let graphite_data = deser(json_80p_of_points_are_below_6());
        let result = assertion.check(&graphite_data);
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
    fn null_points_do_not_count_towards_percent() {
        let assertion = parse_assertion("critical if at least 61% of points are == 0").unwrap();
        let graphite_data = deser(json_all_points_are_0_and_40p_of_points_are_null());
        let result = assertion.check(&graphite_data);
        assert_eq!(result, Status::Critical);
    }

    #[test]
    fn null_points_count_towards_percent_crit() {
        let assertion = parse_assertion("critical if at least 60% of points are == 0").unwrap();
        let graphite_data = deser(json_all_points_are_0_and_40p_of_points_are_null());
        let result = assertion.check(&graphite_data);
        assert_eq!(result, Status::Critical);
    }
}

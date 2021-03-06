use std::str::FromStr;

use clap::{self, value_t};

use tabin_plugins::Status;

use crate::assertions::Assertion;

#[derive(Debug, PartialEq)]
pub(crate) struct Args {
    pub url: String,
    pub path: String,
    pub assertions: Vec<Assertion>,
    pub window: i64,
    pub start_at: i64,
    pub retries: u8,
    pub graphite_error: Status,
    pub no_data: Status,
    pub print_url: bool,
}

lazy_static! {
    // Clap wants its after_help variable to come in as a &str instead of as a
    // String, so we need to make this static variable to be able to pass it
    // between functions. Little bit sad.
    static ref EPILOG: String = format!(
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
                );
}

static ASSERTION_EXAMPLES: &'static [&'static str] = &[
    "critical if any point is > 0",
    "critical if any point in at least 40% of series is > 0",
    "critical if any point is not > 0",
    "warning if any point is == 9",
    "critical if all points are > 100.0",
    "critical if at least 20% of points are > 100",
    "critical if most recent point is > 5",
    "critical if most recent point in all series are == 0",
];

fn build_parser() -> clap::App<'static, 'static> {
    let allowed_no_data = Status::str_values(); // block-local var for borrowck
    clap::App::new("check-graphite (part of tabin-plugins)")
            .version(env!("CARGO_PKG_VERSION"))
            .author("Brandon W Maister <quodlibetor@gmail.com>")
            .setting(clap::AppSettings::ColoredHelp)
            .about("Query graphite and exit based on predicates")
            .args_from_usage(
                "<URL>                  'The domain to query graphite. Must include scheme (http/s)'
                 <PATH>                 'The graphite path to query. For example: \"collectd.*.cpu\"'
                 <ASSERTION>...         'The assertion to make against the PATH. See Below.'
                 -w --window=[MINUTES]  'How many minutes of data to test. Default 10.'
                 --window-start=[MINUTES_IN_PAST] \
                                        'How far back to start the window. Default is now.'
                 --retries=[COUNT]      'How many times to retry reaching graphite. Default 4.'
                 --print-url            'Unconditionally print the graphite url queried'
                 --verify-assertions    'Just check assertion syntax, do not query urls'",
            )
            .arg(
                clap::Arg::with_name("NO_DATA_STATUS")
                    .long("--no-data")
                    .help(
                        "What to do with no data. \
                         This is the value to use for the assertion 'if all values are null' \
                         Default: warn.",
                    )
                    .takes_value(true)
                    .possible_values(&allowed_no_data),
            )
            .arg(
                clap::Arg::with_name("GRAPHITE_ERROR_STATUS")
                    .long("--graphite-error")
                    .help(
                        "What to say if graphite returns a 500 or invalid JSON. \
                         Default: unknown.",
                    )
                    .takes_value(true)
                    .possible_values(&allowed_no_data),
            )
            .after_help(&**EPILOG)
}

impl Args {
    /// Parse all arguments provided at the command line
    pub fn parse() -> Args {
        Args::from_args(build_parser().get_matches())
    }

    #[cfg(test)]
    fn parse_from(args: &[&str]) -> Args {
        Args::from_args(build_parser().get_matches_from(args))
    }

    fn from_args(args: clap::ArgMatches) -> Args {
        let assertions = args
            .values_of("ASSERTION")
            .unwrap()
            .map(|assertion_str| match Assertion::from_str(assertion_str) {
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

        let start_offset = value_t!(args.value_of("window-start"), i64).unwrap_or(0);
        let window = value_t!(args.value_of("window"), i64).unwrap_or(10);
        Args {
            url: args.value_of("URL").unwrap().to_owned(),
            path: args.value_of("PATH").unwrap().to_owned(),
            assertions: assertions,
            window: start_offset + window,
            start_at: start_offset,
            retries: value_t!(args.value_of("retries"), u8).unwrap_or(4),
            graphite_error: Status::from_str(
                args.value_of("GRAPHITE_ERROR_STATUS").unwrap_or("unknown"),
            )
            .unwrap(),
            no_data: Status::from_str(args.value_of("NO_DATA_STATUS").unwrap_or("warning"))
                .unwrap(),
            print_url: args.is_present("print-url"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::assertions::*;

    #[test]
    fn all_examples_are_accurate() {
        for assertion in ASSERTION_EXAMPLES {
            println!("testing `{}`", assertion);
            Assertion::from_str(assertion).unwrap();
        }
    }

    #[test]
    fn args_parse_defaults() {
        let args = Args::parse_from(&[
            "check-graphite-test",
            "https://graphite.example.com",
            "*",
            "critical if any point is > 0",
        ]);
        assert_eq!(
            args,
            Args {
                url: "https://graphite.example.com".into(),
                path: "*".into(),
                assertions: vec![Assertion {
                    operator: ">".into(),
                    op_is_negated: NegOp::No,
                    threshold: 0.0,
                    point_assertion: PointAssertion::Ratio(0.0),
                    series_ratio: 0.0,
                    failure_status: Status::Critical,
                },],
                window: 10,
                start_at: 0,
                retries: 4,
                graphite_error: Status::Unknown,
                no_data: Status::Warning,
                print_url: false,
            }
        )
    }

    #[test]
    fn args_parse_all_values() {
        let args = Args::parse_from(&[
            "check-graphite-test",
            "--window=5",
            "--window-start=20",
            "--retries=7",
            "--graphite-error=ok",
            "--no-data=critical",
            "--print-url",
            "https://graphite.example.com",
            "*",
            "critical if any point is > 0",
        ]);
        assert_eq!(
            args,
            Args {
                url: "https://graphite.example.com".into(),
                path: "*".into(),
                assertions: vec![Assertion {
                    operator: ">".into(),
                    op_is_negated: NegOp::No,
                    threshold: 0.0,
                    point_assertion: PointAssertion::Ratio(0.0),
                    series_ratio: 0.0,
                    failure_status: Status::Critical,
                },],
                window: 25,
                start_at: 20,
                retries: 7,
                graphite_error: Status::Ok,
                no_data: Status::Critical,
                print_url: true,
            }
        )
    }
}

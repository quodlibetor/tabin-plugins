use std::process::Command;

struct Check {
    name: &'static str,
    about: &'static str,
}

fn main() {
    let preamble = "Documentation about the various scripts contained herein\n";

    let checks = [
        Check {
            name: "check-graphite",
            about: "Cross platform, only requires access to a graphite instance.",
        },
        Check {
            name: "check-cpu",
            about: "Linux-only.",
        },
        Check {
            name: "check-container-cpu",
            about: "Linux-only. Can only be run from inside a cgroup.",
        },
        Check {
            name: "check-load",
            about: "Linux-only.",
        },
        Check {
            name: "check-ram",
            about: "Linux-only.",
        },
        Check {
            name: "check-container-ram",
            about: "Linux-only. Can only be run from inside a cgroup.",
        },
        Check {
            name: "check-procs",
            about: "Linux-only. Reads running processes",
        },
        Check {
            name: "check-fs-writeable",
            about: "",
        },
        Check {
            name: "check-disk",
            about: "Unix only.",
        },
    ];

    let mut out: String = cp(preamble.split('\n'));
    out.push_str("\n");
    out.push_str(&cp(checks
        .iter()
        .map(|c| format!("- [{0}](#{0})", c.name))));
    out.push_str("\n");
    for check in &checks {
        out.push_str(&format!(
            "\
//!
//! # {0}
//!
//! {1}
//!
//! ```plain
//! $ {0} --help
",
            check.name, check.about
        ));
        let cout = String::from_utf8(
            Command::new(&format!("target/debug/{}", check.name))
                .args(&["--help"])
                .output()
                .expect(&format!("Couldn't execute command: {}", check.name))
                .stdout,
        ).expect(&format!(
            "Couldn't convert command {} help to utf8",
            check.name
        ));
        out.push_str(&cp(cout.split('\n')));
        out.push_str("\n//! ```\n");
    }
    out.push_str("\n");
    print!("{}", out);
}

/// Comment each line in the iterator
fn cp<S: AsRef<str>, I: Iterator<Item = S>>(s: I) -> String {
    s.map(|s| format!("//! {}", s.as_ref()))
        .map(|s| s.trim().into())
        .collect::<Vec<String>>()
        .join("\n")
}

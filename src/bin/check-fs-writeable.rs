//! Check that we can write to disk

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

use std::fs;
use std::io::Write;
use std::io::ErrorKind;
use std::thread;
use std::path::Path;
use std::process;
use std::sync::mpsc::channel;

use structopt::StructOpt;

/// Check that we can write to a filesystem by writing a byte to a file.
///
/// Does not try to create the directory, or do anything else. Just writes a
/// single byte to a file, errors if it cannot, and then deletes the file.
#[derive(StructOpt, Deserialize)]
struct Args {
    #[structopt(help = "The file to write to")]
    filename: String,
}

fn check_file_writeable(filename: String) -> Result<String, String> {
    let (tx, rx) = channel();
    // We spawn a thread because rust doesn't provide a way to close a file, it
    // just panics if issues happen when it goes out of scope.
    let child = thread::spawn(move || {
        let path = Path::new(&filename);
        match fs::File::create(&path) {
            Err(ref e) => match e.kind() {
                ErrorKind::NotFound => {
                    let dir = path.parent().unwrap_or(Path::new("/"));
                    tx.send(Err(format!(
                        "CRITICAL: directory {} does not exist.",
                        dir.display()
                    ))).unwrap();
                }
                _ => tx.send(Err(format!(
                    "CRITICAL: unexpected error writing to {}: {}",
                    path.display(),
                    e
                ))).unwrap(),
            },
            Ok(mut f) => {
                f.write_all(b"t").unwrap();
                match f.flush() {
                    Ok(()) => {}
                    Err(_) => tx.send(Err(format!(
                        "CRITICAL: Couldn't flush bytes to {}",
                        path.display()
                    ))).unwrap(),
                }
                fs::remove_file(&path).unwrap();
            }
        }
        tx.send(Ok(format!("OK: wrote some bytes to {}", path.display())))
            .unwrap()
    });

    if let Err(kind) = child.join() {
        return Err(format!("CRITICAL: error writing to file: {:?}", kind));
    }
    match rx.recv() {
        Ok(join_result) => {
            return join_result;
        }
        Err(_) => {
            return Err(format!(
                "UNKNOWN: unexpected status receiving info about file writing."
            ));
        }
    };
}

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let args = Args::from_args();
    let mut exit_status = 0;
    match check_file_writeable(args.filename) {
        Ok(msg) => {
            println!("{}", msg);
        }
        Err(msg) => {
            println!("{}", msg);
            exit_status = 2;
        }
    }
    process::exit(exit_status);
}

#[cfg(test)]
mod test {
    use super::Args;
    use structopt::StructOpt;

    #[test]
    fn can_parse_args() {
        Args::from_iter(["arg0", "/tmp"].into_iter());
    }
}

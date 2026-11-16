//! Command-line arguments: `grammata [OPTIONS] [PATH]...`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const HELP: &str = "\
Grammata: a Typst editor with a live preview of the real PDF.

Usage: grammata [OPTIONS] [PATH]...

Arguments:
  [PATH]...  Files to open as tabs. A folder becomes the root of the file panel.
             A path that does not exist yet opens as a new, empty file.

Options:
      --new-window  Start an independent window instead of sending the paths to the
                    one already running. It does not read or save the session.
  -h, --help        Print this help and exit
  -V, --version     Print the version and exit

Launching Grammata while it is already running opens the paths in that window.
";

#[derive(Debug, PartialEq, Eq)]
pub struct Run {
    pub paths: Vec<PathBuf>,
    pub new_window: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Parsed {
    Run(Run),
    Help,
    Version,
}

/// Parses the arguments (without the program name). Relative paths are made
/// absolute against `cwd`. `--` ends the options.
pub fn parse(args: impl IntoIterator<Item = OsString>, cwd: &Path) -> Result<Parsed, String> {
    let mut paths = Vec::new();
    let mut new_window = false;
    let mut only_paths = false;
    for arg in args {
        let text = arg.to_string_lossy();
        if only_paths || !text.starts_with('-') || text == "-" {
            paths.push(cwd.join(&arg));
            continue;
        }
        match text.as_ref() {
            "--" => only_paths = true,
            "--new-window" => new_window = true,
            "-h" | "--help" => return Ok(Parsed::Help),
            "-V" | "--version" => return Ok(Parsed::Version),
            other => return Err(format!("unknown option '{other}'")),
        }
    }
    Ok(Parsed::Run(Run { paths, new_window }))
}

pub fn version() -> String {
    format!("grammata {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Parsed, String> {
        parse(args.iter().map(OsString::from), Path::new("/home/u"))
    }

    #[test]
    fn without_arguments_it_just_runs() {
        assert_eq!(run(&[]), Ok(Parsed::Run(Run { paths: vec![], new_window: false })));
    }

    #[test]
    fn paths_become_absolute() {
        assert_eq!(
            run(&["a.typ", "/etc/b.typ", "sub/c.typ"]),
            Ok(Parsed::Run(Run {
                paths: vec![
                    "/home/u/a.typ".into(),
                    "/etc/b.typ".into(),
                    "/home/u/sub/c.typ".into(),
                ],
                new_window: false,
            }))
        );
    }

    #[test]
    fn new_window_is_a_flag_next_to_the_paths() {
        assert_eq!(
            run(&["--new-window", "a.typ"]),
            Ok(Parsed::Run(Run { paths: vec!["/home/u/a.typ".into()], new_window: true }))
        );
    }

    #[test]
    fn help_and_version() {
        assert_eq!(run(&["--help"]), Ok(Parsed::Help));
        assert_eq!(run(&["a.typ", "-h"]), Ok(Parsed::Help));
        assert_eq!(run(&["-V"]), Ok(Parsed::Version));
        assert_eq!(run(&["--version"]), Ok(Parsed::Version));
    }

    #[test]
    fn unknown_option_is_an_error() {
        assert_eq!(run(&["--nope"]), Err("unknown option '--nope'".into()));
    }

    #[test]
    fn double_dash_allows_names_starting_with_a_dash() {
        assert_eq!(
            run(&["--", "-raro.typ"]),
            Ok(Parsed::Run(Run {
                paths: vec!["/home/u/-raro.typ".into()],
                new_window: false,
            }))
        );
    }

    #[test]
    fn version_names_the_program() {
        assert!(version().starts_with("grammata "));
    }
}

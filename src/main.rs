#![warn(unused_qualifications)]
mod launcher;
mod logging;
mod utils;

use std::{
    panic::{self, PanicHookInfo},
    str::FromStr,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    thread,
};

use anyhow::Result;
use backtrace::Backtrace;
use clap::{crate_description, crate_version, Arg, ArgAction, ArgMatches, Command};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use once_cell::sync::Lazy;
use semver::Version;
use signal_hook::{consts::TERM_SIGNALS, iterator::Signals};
use tracing::warn;

use launcher::Launcher;

const APP_NAME: &str = "Casper node launcher";

/// Global variable holding the PID of the current child process.
static CHILD_PID: Lazy<Arc<AtomicU32>> = Lazy::new(|| Arc::new(AtomicU32::new(0)));

/// Terminates the child process by sending a SIGTERM signal.
fn stop_child() {
    let pid = Pid::from_raw(CHILD_PID.load(Ordering::SeqCst) as i32);
    let _ = signal::kill(pid, Signal::SIGTERM);
}

/// A panic handler which ensures the child process is killed before this process exits.
fn panic_hook(info: &PanicHookInfo) {
    let backtrace = Backtrace::new();

    eprintln!("{backtrace:?}");

    // Print panic info.
    if let Some(&string) = info.payload().downcast_ref::<&str>() {
        eprintln!("node panicked: {string}");
    } else {
        eprintln!("{info}");
    }

    stop_child()
}

/// A signal handler which ensures the child process is killed before this process exits.
fn signal_handler() {
    let mut signals = Signals::new(TERM_SIGNALS).expect("should register signals");
    if signals.forever().next().is_some() {
        stop_child()
    }
}

/// Builds the command line interface.
fn cli() -> Command {
    Command::new(APP_NAME)
        .version(crate_version!())
        .arg(
            Arg::new("force-version")
                .short('f')
                .long("force-version")
                .value_name("version")
                .help("Forces the launcher to run the specified version of the node, for example \"1.2.3\"")
                .value_parser(|arg: &str| {
                    Version::from_str(arg)
                        .map_err(|_| format!("unable to parse '{arg}' as version"))
                })
                .required(false)
                .action(ArgAction::Set),
        )
        .about(crate_description!())
}

/// Extracts the version the launcher was asked to run, if any.
fn forced_version(matches: &ArgMatches) -> Option<Version> {
    matches.get_one::<Version>("force-version").cloned()
}

fn main() -> Result<()> {
    logging::init()?;

    // Create a panic handler.
    panic::set_hook(Box::new(panic_hook));

    // Register signal handlers for SIGTERM, SIGQUIT and SIGINT.  Don't hold on to the joiner for
    // this thread as it will block if the child process dies without a signal having been received
    // in the main launcher process.
    let _ = thread::spawn(signal_handler);

    let matches = cli().get_matches();
    let forced_version = forced_version(&matches);

    let mut launcher = Launcher::new(forced_version)?;
    launcher.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches_from(args: &[&str]) -> Result<ArgMatches, clap::Error> {
        cli().try_get_matches_from(args)
    }

    #[test]
    fn should_parse_forced_version() {
        let matches = matches_from(&["casper-node-launcher", "--force-version", "1.2.3"]).unwrap();
        assert_eq!(forced_version(&matches), Some(Version::new(1, 2, 3)));
    }

    #[test]
    fn should_have_no_forced_version_by_default() {
        let matches = matches_from(&["casper-node-launcher"]).unwrap();
        assert_eq!(forced_version(&matches), None);
    }

    #[test]
    fn should_reject_invalid_forced_version() {
        let error = matches_from(&["casper-node-launcher", "--force-version", "not-a-version"])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("unable to parse 'not-a-version' as version"),
            "unexpected error: {}",
            error
        );
    }
}

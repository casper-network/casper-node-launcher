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
use clap::{crate_description, crate_version, Arg, Command};
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

fn main() -> Result<()> {
    logging::init()?;

    // Create a panic handler.
    panic::set_hook(Box::new(panic_hook));

    // Register signal handlers for SIGTERM, SIGQUIT and SIGINT.  Don't hold on to the joiner for
    // this thread as it will block if the child process dies without a signal having been received
    // in the main launcher process.
    let _ = thread::spawn(signal_handler);
    let command = Command::new(APP_NAME)
        .version(crate_version!())
        .arg(
            Arg::new("force-version")
                .short('f')
                .long("force-version")
                .value_name("version")
                .help("Forces the launcher to run the specified version of the node, for example \"1.2.3\"")
                .validator(|arg: &str| Version::from_str(arg).map_err(|_| format!("unable to parse '{arg}' as version")))
                .required(false)
                .takes_value(true),
        )
        .about(crate_description!());
    let matches = command.get_matches();

    // Safe to unwrap() as we have the string validated by `clap` already.
    let forced_version = matches
        .value_of("force-version")
        .map(|ver| Version::from_str(ver).unwrap());

    let mut launcher = Launcher::new(forced_version)?;
    launcher.run()
}

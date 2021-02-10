#![warn(unused_qualifications)]
mod launcher;
mod logging;
mod utils;

use std::{
    panic::{self, PanicInfo},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    thread,
};

use anyhow::Result;
use backtrace::Backtrace;
use clap::{crate_description, crate_version, App};
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use once_cell::sync::Lazy;
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
fn panic_hook(info: &PanicInfo) {
    let backtrace = Backtrace::new();

    eprintln!("{:?}", backtrace);

    // Print panic info.
    if let Some(&string) = info.payload().downcast_ref::<&str>() {
        eprintln!("node panicked: {}", string);
    } else {
        eprintln!("{}", info);
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

    let app = App::new(APP_NAME)
        .version(crate_version!())
        .about(crate_description!());
    let _ = app.get_matches();

    let mut launcher = Launcher::new()?;
    launcher.run()
}

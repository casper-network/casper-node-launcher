#![warn(unused_qualifications)]

mod launcher;
mod logging;
mod utils;

use anyhow::Result;
use clap::{crate_description, crate_version, App};
use tracing::warn;

use launcher::Launcher;

const APP_NAME: &str = "Casper node launcher";

fn main() -> Result<()> {
    logging::init()?;

    let app = App::new(APP_NAME)
        .version(crate_version!())
        .about(crate_description!());
    let _ = app.get_matches();

    let mut launcher = Launcher::new()?;
    launcher.run()
}

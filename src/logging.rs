use std::{env, io};

use anyhow::{Error, Result};

use tracing_subscriber::EnvFilter;

const LOG_ENV_VAR: &str = "RUST_LOG";
const DEFAULT_LOG_LEVEL: &str = "info";

pub fn init() -> Result<()> {
    let filter = EnvFilter::new(
        env::var(LOG_ENV_VAR)
            .as_deref()
            .unwrap_or(DEFAULT_LOG_LEVEL),
    );

    Ok(tracing_subscriber::fmt()
        .with_writer(io::stdout)
        .with_env_filter(filter)
        .json()
        .try_init()
        .map_err(Error::msg)?)
}

#[cfg(not(test))]
use std::env;
#[cfg(test)]
use std::thread;
use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
#[cfg(test)]
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use tempfile::TempDir;
use tracing::{debug, info, warn};

/// The name of the config file for the node-launcher.
const CONFIG_NAME: &str = "casper-node-launcher-config.toml";

/// The folder where config files are installed by default.
#[cfg(not(test))]
const DEFAULT_CONFIG_DIR: &str = "/etc/casper";
/// The name of the config file for the current casper-node.
const NODE_CONFIG_NAME: &str = "config.toml";
/// The name of the config file for the next version of casper-node.
const NODE_CONFIG_NEXT_NAME: &str = "config-next.toml";
/// Environment variable to override the default config dir.
#[cfg(not(test))]
const DEFAULT_CONFIG_DIR_OVERRIDE: &str = "CASPER_CONFIG_DIR";

/// The folder where casper-node binaries are installed by default.
#[cfg(not(test))]
const DEFAULT_BINARY_DIR: &str = "/var/lib/casper/bin";
/// The name of the current casper-node binary.
const NODE_BINARY_NAME: &str = "casper-node";
/// The name of the next version of the casper-node binary.
const NODE_BINARY_NEXT_NAME: &str = "casper-node-next";
/// Environment variable to override the default binary dir.
#[cfg(not(test))]
const DEFAULT_BINARY_DIR_OVERRIDE: &str = "CASPER_BIN_DIR";

/// This "leaks" the tempdir, in that it won't be removed after the tests finish running.  However,
/// it is only ever used for testing very small files, and it makes the production code and test
/// code simpler, so it's a worthwhile trade off.
#[cfg(test)]
static TEMP_DIR: Lazy<TempDir> = Lazy::new(|| tempfile::tempdir().expect("should create temp dir"));

#[derive(PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct Config {
    node_config_path: PathBuf,
}

impl Config {
    /// If `maybe_node_config_path` is `Some`:
    ///   * checks the node's config file exists at the given path (returns error if not)
    ///   * writes the path value to the casper-node-launcher's config file (returns an error if it
    ///     can't be written)
    ///
    /// If `maybe_node_config_path` is `None`:
    ///   * if the casper-node-launcher's config file can be read and parsed:
    ///     * checks the node's config file exists at the given path (returns error if not)
    ///     * returns the read-in config
    ///   * otherwise follows the steps for `maybe_node_config_path` is `Some` above, using the
    ///     default value for the node config path
    pub fn new(maybe_node_config_path: Option<&str>) -> Result<Self> {
        if let Some(node_config_path) = maybe_node_config_path {
            if !Path::new(node_config_path).is_file() {
                warn!(path=%node_config_path, "node config missing");
                bail!("node config file doesn't exist at {}", node_config_path);
            }

            let config = Config {
                node_config_path: PathBuf::from(node_config_path),
            };
            config.write()?;

            return Ok(config);
        }

        match Config::read() {
            Ok(config) => {
                if !Path::new(&config.node_config_path).is_file() {
                    warn!(path=%config.node_config_path.display(), "node config doesn't exist");
                    bail!(
                        "stored value invalid: node config file doesn't exist at {}",
                        config.node_config_path.display()
                    );
                }

                return Ok(config);
            }
            Err(error) => {
                if Self::self_path().is_file() {
                    warn!(%error, path=%Self::self_path().display(), "failed to read as config");
                    return Err(error);
                }
            }
        }

        Self::new(Some(
            &Self::default_node_config_path().display().to_string(),
        ))
    }

    /// Provides the actual path of the config file for the current version of casper-node.
    pub fn node_config_path(&self) -> &Path {
        &self.node_config_path
    }

    /// Provides the path of the config file for the next version of casper-node.
    pub fn node_config_next_path(&self) -> PathBuf {
        self.node_config_path
            .parent()
            .unwrap()
            .join(NODE_CONFIG_NEXT_NAME)
    }

    /// Provides the default path of the config file for the casper-node.
    pub fn default_node_config_path() -> PathBuf {
        Self::default_config_dir().join(NODE_CONFIG_NAME)
    }

    /// Provides the path of the current version of the casper-node binary.
    pub fn node_binary_path() -> PathBuf {
        Self::default_binary_dir().join(NODE_BINARY_NAME)
    }

    /// Provides the path of the next version of the casper-node binary.
    pub fn node_binary_next_path() -> PathBuf {
        Self::default_binary_dir().join(NODE_BINARY_NEXT_NAME)
    }

    /// Provides the default path of the config dir.
    ///
    /// For `test` configuration, this is a folder named `config` inside a folder in the `TEMP_DIR`
    /// named as per the individual test's thread.
    ///
    /// Otherwise it is `/etc/casper`, although this can be overridden (e.g. for external tests), by
    /// setting the env var `CASPER_CONFIG_DIR` to a different folder.
    fn default_config_dir() -> PathBuf {
        #[cfg(not(test))]
        {
            PathBuf::from(match env::var(DEFAULT_CONFIG_DIR_OVERRIDE) {
                Ok(path) => path,
                Err(_) => DEFAULT_CONFIG_DIR.to_string(),
            })
        }
        #[cfg(test)]
        {
            let path = TEMP_DIR
                .path()
                .join(thread::current().name().unwrap())
                .join("config");
            let _ = fs::create_dir_all(&path);
            path
        }
    }

    /// Provides the default path of the binary dir.
    ///
    /// For `test` configuration, this is a folder named `bin` inside a folder in the `TEMP_DIR`
    /// named as per the individual test's thread.
    ///
    /// Otherwise it is `/var/lib/casper/bin`, although this can be overridden (e.g. for external
    /// tests), by setting the env var `CASPER_BIN_DIR` to a different folder.
    fn default_binary_dir() -> PathBuf {
        #[cfg(not(test))]
        {
            PathBuf::from(match env::var(DEFAULT_BINARY_DIR_OVERRIDE) {
                Ok(path) => path,
                Err(_) => DEFAULT_BINARY_DIR.to_string(),
            })
        }
        #[cfg(test)]
        {
            let path = TEMP_DIR
                .path()
                .join(thread::current().name().unwrap_or("unnamed"))
                .join("bin");
            let _ = fs::create_dir_all(&path);
            path
        }
    }

    /// Provides the path of the config file for the node-launcher.
    fn self_path() -> PathBuf {
        Self::default_config_dir().join(CONFIG_NAME)
    }

    /// Constructs a new `Config` by reading it in from the hard-coded location.
    fn read() -> Result<Self> {
        let path = Self::self_path();
        debug!(path=%path.display(), "trying to read config");
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config = toml::from_str(&contents)?;
        info!(path=%path.display(), "read config");
        Ok(config)
    }

    /// Writes `self` to the hard-coded location as a TOML-encoded file.
    fn write(&self) -> Result<()> {
        let path = Self::self_path();
        debug!(path=%path.display(), "trying to write config");
        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents.as_bytes())
            .with_context(|| format!("failed to write {}", path.display()))?;
        info!(path=%path.display(), "wrote config");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging;

    #[test]
    fn should_not_write_if_node_config_does_not_exist() {
        let _ = logging::init();

        // Try with default for node config.
        let error = Config::new(None).unwrap_err();
        let error_msg = error.downcast_ref::<String>().unwrap();
        assert!(
            error_msg.starts_with("node config file doesn't exist at"),
            "{}",
            error_msg
        );
        assert!(
            error_msg.contains(&format!("/{}", NODE_CONFIG_NAME)),
            "{}",
            error_msg
        );

        // Try with a non-default path which also doesn't exist.
        let bad_path = "bad-path";
        let error = Config::new(Some(bad_path)).unwrap_err();
        let error_msg = error.downcast_ref::<String>().unwrap();
        assert!(
            error_msg.starts_with("node config file doesn't exist at"),
            "{}",
            error_msg
        );
        assert!(error_msg.contains(bad_path), "{}", error_msg);
    }

    #[test]
    fn should_write_if_node_config_exists() {
        let _ = logging::init();

        // Try with default for node config.
        // Create the node's config file.
        fs::write(Config::default_node_config_path(), [1]).unwrap();
        // Create the node-launcher's config file.
        let config = Config::new(None).unwrap();
        assert_eq!(Config::default_node_config_path(), config.node_config_path);
        let read_config = Config::read().unwrap();
        assert_eq!(config, read_config);

        // Try with a non-default path.
        let new_path = Config::default_config_dir().join("new-path");
        // Create the node's config file.
        fs::write(&new_path, [1]).unwrap();
        // Create the node-launcher's config file.
        let config = Config::new(Some(&new_path.display().to_string())).unwrap();
        assert_eq!(new_path, config.node_config_path);
        let read_config = Config::read().unwrap();
        assert_eq!(config, read_config);
    }

    #[test]
    fn should_not_write_if_config_corrupted() {
        let _ = logging::init();

        // Create the node's config file at the default location.
        fs::write(Config::default_node_config_path(), [1]).unwrap();
        // Create a corrupted node-launcher config file.
        fs::write(Config::self_path(), [1]).unwrap();

        // Check we get a toml error returned.
        let error = Config::new(None).unwrap_err();
        assert!(error.downcast_ref::<toml::de::Error>().is_some());
    }
}

#[cfg(not(test))]
use std::env;
#[cfg(test)]
use std::thread;
use std::{fs, mem, path::PathBuf, process::Command};

use anyhow::{bail, Result};
#[cfg(test)]
use once_cell::sync::Lazy;
use semver::Version;
use serde::{Deserialize, Serialize};
#[cfg(test)]
use tempfile::TempDir;
use tracing::{debug, info, warn};

use crate::utils;

/// The name of the file for the on-disk record of the node-launcher's current state.
const STATE_FILE_NAME: &str = "casper-node-launcher-state.toml";

/// The folder under which casper-node binaries are installed.
#[cfg(not(test))]
const BINARY_ROOT_DIR: &str = "/var/lib/casper/bin";
/// The name of the casper-node binary.
const NODE_BINARY_NAME: &str = "casper-node";
/// Environment variable to override the binary root dir.
#[cfg(not(test))]
const BINARY_ROOT_DIR_OVERRIDE: &str = "CASPER_BIN_DIR";

/// The folder under which config files are installed.
#[cfg(not(test))]
const CONFIG_ROOT_DIR: &str = "/etc/casper";
/// The name of the config file for casper-node.
const NODE_CONFIG_NAME: &str = "config.toml";
/// Environment variable to override the config root dir.
#[cfg(not(test))]
const CONFIG_ROOT_DIR_OVERRIDE: &str = "CASPER_CONFIG_DIR";

/// The subcommands and args for casper-node.
const MIGRATE_SUBCOMMAND: &str = "migrate-data";
const OLD_CONFIG_ARG: &str = "--old-config";
const NEW_CONFIG_ARG: &str = "--new-config";
const VALIDATOR_SUBCOMMAND: &str = "validator";

/// This "leaks" the tempdir, in that it won't be removed after the tests finish running.  However,
/// it is only ever used for testing very small files, and it makes the production code and test
/// code simpler, so it's a worthwhile trade off.
#[cfg(test)]
static TEMP_DIR: Lazy<TempDir> = Lazy::new(|| tempfile::tempdir().expect("should create temp dir"));

/// Details of the node and its files.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct NodeInfo {
    /// The version of the node software.
    pub version: Version,
    /// The path to the node binary.
    pub binary_path: PathBuf,
    /// The path to the node's config file.
    pub config_path: PathBuf,
}

/// The state of the launcher, cached to disk every time it changes.
#[derive(PartialEq, Eq, Serialize, Deserialize, Debug)]
#[serde(tag = "mode")]
enum State {
    RunNodeAsValidator(NodeInfo),
    MigrateData {
        old_info: NodeInfo,
        new_info: NodeInfo,
    },
}

impl Default for State {
    fn default() -> Self {
        let node_info = NodeInfo {
            version: Version::new(0, 0, 0),
            binary_path: PathBuf::new(),
            config_path: PathBuf::new(),
        };
        State::RunNodeAsValidator(node_info)
    }
}

/// The object responsible for running the casper-node as a child process.
///
/// It operates as a state machine, iterating between running the node in validator mode and running
/// it in migrate-data mode.
///
/// At each state transition, it caches its state to disk so that it can resume the same operation
/// if restarted.
#[derive(PartialEq, Eq, Debug)]
pub struct Launcher {
    binary_root_dir: PathBuf,
    config_root_dir: PathBuf,
    state: State,
}

impl Launcher {
    /// Constructs a new `Launcher`.
    ///
    /// If the launcher was previously run, this will try and parse its previous state.  Otherwise
    /// it will search for the latest installed version of casper-node and start running it in
    /// validator mode.
    pub fn new() -> Result<Self> {
        let mut launcher = Launcher {
            binary_root_dir: Self::binary_root_dir(),
            config_root_dir: Self::config_root_dir(),
            state: State::default(),
        };

        let state_path = launcher.state_path();
        if state_path.exists() {
            debug!(path=%state_path.display(), "trying to read stored state");
            let contents = utils::map_and_log_error(
                fs::read_to_string(&state_path),
                format!("failed to read {}", state_path.display()),
            )?;

            launcher.state = utils::map_and_log_error(
                toml::from_str(&contents),
                format!("failed to parse {}", state_path.display()),
            )?;
            info!(path=%state_path.display(), "read stored state");
            return Ok(launcher);
        }

        debug!(path=%state_path.display(), "stored state doesn't exist");

        let version = launcher.next_installed_version(&Version::new(0, 0, 0))?;
        let node_info = launcher.new_node_info(version);
        launcher.state = State::RunNodeAsValidator(node_info);
        launcher.write()?;
        Ok(launcher)
    }

    /// Runs the launcher, blocking indefinitely.
    pub fn run(&mut self) -> Result<()> {
        loop {
            self.step()?;
        }
    }

    /// Provides the path of the file for recording the state of the node-launcher.
    fn state_path(&self) -> PathBuf {
        self.config_root_dir.join(STATE_FILE_NAME)
    }

    /// Writes `self` to the hard-coded location as a TOML-encoded file.
    fn write(&self) -> Result<()> {
        let path = self.state_path();
        debug!(path=%path.display(), "trying to store state");
        let contents = utils::map_and_log_error(
            toml::to_string_pretty(&self.state),
            "failed to encode state as toml".to_string(),
        )?;
        utils::map_and_log_error(
            fs::write(&path, contents.as_bytes()),
            format!("failed to write {}", path.display()),
        )?;
        info!(path=%path.display(), state=?self.state, "stored state");
        Ok(())
    }

    /// Gets the next installed version of the node binary and config.
    ///
    /// Returns an error if the versions cannot be deduced, or if the two versions are different.
    fn next_installed_version(&self, current_version: &Version) -> Result<Version> {
        let next_binary_version =
            utils::next_installed_version(&self.binary_root_dir, current_version)?;
        let next_config_version =
            utils::next_installed_version(&self.config_root_dir, current_version)?;
        if next_config_version != next_binary_version {
            warn!(%next_binary_version, %next_config_version, "next version mismatch");
            bail!(
                "next binary version {} != next config version {}",
                next_binary_version,
                next_config_version,
            );
        }
        Ok(next_binary_version)
    }

    /// Constructs a new `NodeInfo` based on the given version.
    fn new_node_info(&self, version: Version) -> NodeInfo {
        let subdir_name = version.to_string().replace(".", "_");
        NodeInfo {
            version,
            binary_path: self
                .binary_root_dir
                .join(&subdir_name)
                .join(NODE_BINARY_NAME),
            config_path: self
                .config_root_dir
                .join(&subdir_name)
                .join(NODE_CONFIG_NAME),
        }
    }

    /// Provides the path to the binary root folder.  casper-node binaries will be installed in a
    /// subdir of this path, where the subdir will be named as per the casper-node version.
    ///
    /// For `test` configuration, this is a folder named `bin` inside a folder in the `TEMP_DIR`
    /// named as per the individual test's thread.
    ///
    /// Otherwise it is `/var/lib/casper/bin`, although this can be overridden (e.g. for external
    /// tests), by setting the env var `CASPER_BIN_DIR` to a different folder.
    fn binary_root_dir() -> PathBuf {
        #[cfg(not(test))]
        {
            PathBuf::from(match env::var(BINARY_ROOT_DIR_OVERRIDE) {
                Ok(path) => path,
                Err(_) => BINARY_ROOT_DIR.to_string(),
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

    /// Provides the path to the config root folder.  Config files will be installed in a subdir of
    /// this path, where the subdir will be named as per the casper-node version.
    ///
    /// For `test` configuration, this is a folder named `config` inside a folder in the `TEMP_DIR`
    /// named as per the individual test's thread.
    ///
    /// Otherwise it is `/etc/casper`, although this can be overridden (e.g. for external tests), by
    /// setting the env var `CASPER_CONFIG_DIR` to a different folder.
    fn config_root_dir() -> PathBuf {
        #[cfg(not(test))]
        {
            PathBuf::from(match env::var(CONFIG_ROOT_DIR_OVERRIDE) {
                Ok(path) => path,
                Err(_) => CONFIG_ROOT_DIR.to_string(),
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

    /// Moves the launcher state forward.  If it's currently `RunNodeAsValidator`, then finds the
    /// highest installed version and moves to `MigrateData` if that version is newer (else errors).
    /// If it's currently `MigrateData`, moves to `RunNodeAsValidator` using the newest version.
    fn transition_state(&mut self) -> Result<()> {
        let new_state = match mem::take(&mut self.state) {
            State::RunNodeAsValidator(old_info) => {
                let next_version = self.next_installed_version(&old_info.version)?;
                if next_version <= old_info.version {
                    let msg = format!(
                        "no higher version than current {} installed",
                        old_info.version
                    );
                    warn!("{}", msg);
                    bail!(msg);
                }

                let new_info = self.new_node_info(next_version);
                State::MigrateData { old_info, new_info }
            }
            State::MigrateData { new_info, .. } => State::RunNodeAsValidator(new_info),
        };

        self.state = new_state;
        self.write()?;
        Ok(())
    }

    /// Runs the process for the current state and moves the state forward if the process exits with
    /// success.
    fn step(&mut self) -> Result<()> {
        match &self.state {
            State::RunNodeAsValidator(node_info) => {
                let mut command = Command::new(&node_info.binary_path);
                command
                    .arg(VALIDATOR_SUBCOMMAND)
                    .arg(&node_info.config_path);
                utils::run_command(command)?;
                info!(version=%node_info.version, "finished running node as validator");
            }
            State::MigrateData { old_info, new_info } => {
                let mut command = Command::new(&new_info.binary_path);
                command
                    .arg(MIGRATE_SUBCOMMAND)
                    .arg(OLD_CONFIG_ARG)
                    .arg(&old_info.config_path)
                    .arg(NEW_CONFIG_ARG)
                    .arg(&new_info.config_path);
                utils::run_command(command)?;
                info!(
                    old_version=%old_info.version,
                    new_version=%new_info.version,
                    "finished data migration"
                );
            }
        }

        self.transition_state()
    }
}

#[cfg(test)]
mod tests {
    use std::{os::unix::fs::PermissionsExt, thread, time::Duration};

    use super::*;
    use crate::logging;

    const NODE_CONTENTS: &str = include_str!("../test_resources/casper-node.in");
    /// The duration to wait after starting a mock casper-node instance before "installing" a new
    /// version of the mock node.  The mock sleeps for 1 second while running in validator mode, so
    /// 100ms should be enough to allow the node-launcher step to start.
    const DELAY_INSTALL_DURATION: Duration = Duration::from_millis(100);
    static V1: Lazy<Version> = Lazy::new(|| Version::new(1, 0, 0));
    static V2: Lazy<Version> = Lazy::new(|| Version::new(2, 0, 0));
    static V3: Lazy<Version> = Lazy::new(|| Version::new(3, 0, 0));

    /// Installs the new version of the mock node binary, assigning an old version for the script
    /// with the major version of `new_version` decremented by 1.
    fn install_mock(new_version: &Version) {
        if thread::current().name().is_none() {
            panic!(
                "install_mock must be called from the main test thread in order for \
                `Launcher::binary_root_dir()` and `Launcher::config_root_dir()` to work"
            );
        }

        let subdir_name = new_version.to_string().replace(".", "_");

        // Create the node script contents.
        let old_version = Version::new(new_version.major - 1, new_version.minor, new_version.patch);
        let node_contents = NODE_CONTENTS.replace(
            r#"OLD_VERSION="""#,
            &format!(r#"OLD_VERSION="{}""#, old_version),
        );
        let node_contents =
            node_contents.replace(r#"VERSION="""#, &format!(r#"VERSION="{}""#, new_version));

        // Create the subdir for the node binary.
        let binary_folder = Launcher::binary_root_dir().join(&subdir_name);
        fs::create_dir(&binary_folder).unwrap();

        // Create the node script as an executable file.
        let binary_path = binary_folder.join(NODE_BINARY_NAME);
        fs::write(&binary_path, node_contents.as_bytes()).unwrap();
        let mut permissions = fs::metadata(&binary_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary_path, permissions).unwrap();

        // Create the subdir for the node config.
        let config_folder = Launcher::config_root_dir().join(&subdir_name);
        fs::create_dir(&config_folder).unwrap();

        // Create the node config file containing only the version.
        let config_path = config_folder.join(NODE_CONFIG_NAME);
        fs::write(&config_path, new_version.to_string().as_bytes()).unwrap();
    }

    /// Asserts that `line` equals the last line in the log file which the mock casper-node should
    /// have written to.
    fn assert_last_log_line_eq(launcher: &Launcher, line: &str) {
        let log_path = launcher.binary_root_dir.parent().unwrap().join("log.txt");
        let log_contents = fs::read_to_string(&log_path).unwrap();
        assert_eq!(line, log_contents.lines().last().unwrap());
    }

    /// Asserts that the last line in the log file which the mock casper-node should have written to
    /// contains the provided string.
    fn assert_last_log_line_contains(launcher: &Launcher, string: &str) {
        let log_path = launcher.binary_root_dir.parent().unwrap().join("log.txt");
        let log_contents = fs::read_to_string(&log_path).unwrap();
        let last_line = log_contents.lines().last().unwrap();
        assert!(
            last_line.contains(string),
            "'{}' doesn't contain '{}'",
            last_line,
            string
        );
    }

    #[test]
    fn should_write_state_on_first_run() {
        let _ = logging::init();

        install_mock(&*V1);
        let launcher = Launcher::new().unwrap();
        assert!(launcher.state_path().exists());

        // Check the state was stored to disk.
        let toml_contents = fs::read_to_string(&launcher.state_path()).unwrap();
        let stored_state = toml::from_str(&toml_contents).unwrap();
        assert_eq!(launcher.state, stored_state);

        // Check the stored state is as expected.
        let expected_node_info = launcher.new_node_info(V1.clone());
        let expected_state = State::RunNodeAsValidator(expected_node_info);
        assert_eq!(expected_state, stored_state);
    }

    #[test]
    fn should_read_state_on_startup() {
        let _ = logging::init();

        // Write the state to disk (RunNodeAsValidator for V1).
        install_mock(&*V1);
        let _ = Launcher::new().unwrap();

        // Install a new version of node, but ensure a new launcher reads the state from disk rather
        // than detecting a new version.
        install_mock(&*V2);
        let launcher = Launcher::new().unwrap();

        let expected_node_info = launcher.new_node_info(V1.clone());
        let expected_state = State::RunNodeAsValidator(expected_node_info);
        assert_eq!(expected_state, launcher.state);
    }

    #[test]
    fn should_error_if_state_corrupted() {
        let _ = logging::init();

        // Write the state to disk (RunNodeAsValidator for V1).
        install_mock(&*V1);
        let launcher = Launcher::new().unwrap();

        // Corrupt the stored state.
        fs::write(&launcher.state_path(), "bad value".as_bytes()).unwrap();
        let error = Launcher::new().unwrap_err().to_string();
        assert_eq!(
            format!("failed to parse {}", launcher.state_path().display()),
            error
        );
    }

    #[test]
    fn should_error_if_node_not_installed_on_first_run() {
        let _ = logging::init();

        let error = Launcher::new().unwrap_err().to_string();
        assert_eq!(
            format!(
                "failed to get a valid version from subdirs in {}",
                Launcher::binary_root_dir().display()
            ),
            error
        );
    }

    #[test]
    fn should_run_upgrades() {
        let _ = logging::init();

        // Set up the test folders as if casper-node has just been installed at v1.0.0.
        install_mock(&*V1);

        // Set up a thread to run the launcher's first two steps.
        let mut launcher = Launcher::new().unwrap();
        let worker = thread::spawn(move || {
            // Run the launcher's first step - should run node v1.0.0 in validator mode, taking 1
            // second to complete.
            launcher.step().unwrap();
            assert_last_log_line_eq(&launcher, "Node v1.0.0 ran as validator");

            // Run the launcher's second step - should run node v2.0.0 in data-migration mode,
            // completing immediately.
            launcher.step().unwrap();
            assert_last_log_line_eq(&launcher, "Node v2.0.0 migrated data");

            launcher
        });

        // Install node v2.0.0 after v1.0.0 has started running.
        thread::sleep(DELAY_INSTALL_DURATION);
        install_mock(&*V2);

        launcher = worker.join().unwrap();

        // Set up a thread to run the launcher's next two steps.
        let worker = thread::spawn(move || {
            // Run the launcher's third step - should run node v2.0.0 in validator mode, taking 1
            // second to complete.
            launcher.step().unwrap();
            assert_last_log_line_eq(&launcher, "Node v2.0.0 ran as validator");

            // Run the launcher's fourth step - should run node v3.0.0 in data-migration mode,
            // completing immediately.
            launcher.step().unwrap();
            assert_last_log_line_eq(&launcher, "Node v3.0.0 migrated data");

            launcher
        });

        // Install node v3.0.0 after v2.0.0 has started running.
        thread::sleep(DELAY_INSTALL_DURATION);
        install_mock(&*V3);

        launcher = worker.join().unwrap();

        // Run the launcher's fifth step - should run node v3.0.0 in validator mode.  As there
        // will be no further upgraded binary available after the node exits, the step should return
        // an error.
        let error = launcher.step().unwrap_err().to_string();
        assert_last_log_line_eq(&launcher, "Node v3.0.0 ran as validator");
        assert_eq!("no higher version than current 3.0.0 installed", error);
    }

    #[test]
    fn should_not_upgrade_to_lower_version() {
        let _ = logging::init();

        install_mock(&*V2);

        // Set up a thread to run the launcher.
        let mut launcher = Launcher::new().unwrap();
        let worker = thread::spawn(move || {
            // Run the launcher's first step - should run node v2.0.0 in validator mode, taking 1
            // second to complete, but then fail to find a newer installed version.
            let error = launcher.step().unwrap_err().to_string();
            assert_last_log_line_eq(&launcher, "Node v2.0.0 ran as validator");
            assert_eq!("no higher version than current 2.0.0 installed", error);
        });

        // Install node v1.0.0 after v2.0.0 has started running.
        thread::sleep(DELAY_INSTALL_DURATION);
        install_mock(&*V1);

        worker.join().unwrap();
    }

    #[test]
    fn should_run_again_after_crash_while_in_validator_mode() {
        let _ = logging::init();

        // Set up the test folders as if casper-node has just been installed, but provide a config
        // file which will cause the node to crash as soon as it starts.
        install_mock(&*V1);
        let mut launcher = Launcher::new().unwrap();
        let node_info = launcher.new_node_info(V1.clone());
        let bad_value = "bad value";
        fs::write(&node_info.config_path, bad_value.as_bytes()).unwrap();

        // Run the launcher step - should return an error indicating the node exited with an
        // error, but should leave the node and config unchanged and still runnable.
        let error = launcher.step().unwrap_err().to_string();
        assert_last_log_line_contains(
            &launcher,
            &format!("should contain 1.0.0 but contains {}", bad_value),
        );
        assert!(error.ends_with("exited with error"), "{}", error);

        // Fix the config file to be valid and try running the node again.  The launcher will
        // error out again, but this time after the node has finished running in validator mode due
        // to there being no upgraded binary available after the node exits.
        fs::write(&node_info.config_path, V1.to_string().as_bytes()).unwrap();
        let error = launcher.step().unwrap_err().to_string();
        assert_last_log_line_eq(&launcher, "Node v1.0.0 ran as validator");
        assert_eq!("no higher version than current 1.0.0 installed", error);
    }

    #[test]
    fn should_run_again_after_crash_while_in_migration_mode() {
        let _ = logging::init();

        // Set up the test folders as if casper-node has just been installed.
        install_mock(&*V1);

        // Set up a thread to run the launcher's first two steps.
        let mut launcher = Launcher::new().unwrap();
        let node_v2_info = launcher.new_node_info(V2.clone());
        let bad_value = "bad value";
        let worker = thread::spawn(move || {
            // Run the launcher's first step - should run node v1.0.0 in validator mode, taking 1
            // second to complete.
            launcher.step().unwrap();
            assert_last_log_line_eq(&launcher, "Node v1.0.0 ran as validator");

            // Run the launcher's second step - should run node v2.0.0 in data-migration mode, but
            // should return an error indicating the node exited with an error, and should leave the
            // node and config unchanged and still runnable.
            let error = launcher.step().unwrap_err().to_string();
            assert_last_log_line_contains(
                &launcher,
                &format!("should contain 2.0.0 but contains {}", bad_value),
            );
            assert!(error.ends_with("exited with error"), "{}", error);

            launcher
        });

        // Install node v2.0.0 after v1.0.0 has started running, but provide a config file which
        // will cause the node to crash as soon as it starts.
        thread::sleep(DELAY_INSTALL_DURATION);
        install_mock(&*V2);
        fs::write(&node_v2_info.config_path, bad_value.as_bytes()).unwrap();

        launcher = worker.join().unwrap();

        // Fix the config file to be valid and try running the node again.  It should run in data-
        // migration mode again.
        fs::write(&node_v2_info.config_path, V2.to_string().as_bytes()).unwrap();
        launcher.step().unwrap();
        assert_last_log_line_eq(&launcher, "Node v2.0.0 migrated data");
    }

    #[test]
    fn should_error_if_bin_and_config_have_different_versions() {
        let _ = logging::init();

        install_mock(&*V1);
        // Rename the config folder to 2_0_0.
        fs::rename(
            Launcher::config_root_dir().join("1_0_0"),
            Launcher::config_root_dir().join("2_0_0"),
        )
        .unwrap();

        let error = Launcher::new().unwrap_err().to_string();
        assert_eq!(
            "next binary version 1.0.0 != next config version 2.0.0",
            error
        );
    }
}

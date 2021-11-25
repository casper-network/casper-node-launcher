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

use crate::utils::{self, NodeExitCode};

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
        let installed_binary_versions = utils::versions_from_path(&Self::binary_root_dir())?;
        let installed_config_versions = utils::versions_from_path(&Self::config_root_dir())?;

        if installed_binary_versions != installed_config_versions {
            bail!(
                "installed binary versions ({}) don't match installed configs ({})",
                utils::iter_to_string(installed_binary_versions),
                utils::iter_to_string(installed_config_versions),
            );
        }

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

        let version = launcher.most_recent_version()?;
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

    /// Gets the most recent installed binary version.
    ///
    /// Returns an error when no correct versions can be detected.
    fn most_recent_version(&self) -> Result<Version> {
        let all_versions = utils::versions_from_path(&Self::binary_root_dir())?;

        // We are guaranteed to have at least one version in the `all_versions` container,
        // because if there are no valid version installed the `utils::versions_from_path()` will bail.
        if all_versions.is_empty() {}

        if let Some(most_recent_version) = all_versions.into_iter().last() {
            Ok(most_recent_version)
        } else {
            // `utils::versions_from_path()` will log a message for us
            unreachable!();
        }
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

    /// Gets the previous installed version of the node binary and config.
    ///
    /// Returns an error if the versions cannot be deduced, or if the two versions are different.
    fn previous_installed_version(&self, current_version: &Version) -> Result<Version> {
        let previous_binary_version =
            utils::previous_installed_version(&self.binary_root_dir, current_version)?;
        let previous_config_version =
            utils::previous_installed_version(&self.config_root_dir, current_version)?;
        if previous_config_version != previous_binary_version {
            warn!(%previous_binary_version, %previous_config_version, "previous version mismatch");
            bail!(
                "previous binary version {} != previous config version {}",
                previous_binary_version,
                previous_config_version,
            );
        }
        Ok(previous_binary_version)
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

    /// Sets `self.state` to a new state corresponding to upgrading the current node version.
    ///
    /// If `self.state` is currently `RunNodeAsValidator`, then finds the next installed version
    /// and moves to `MigrateData` if that version is newer (else errors).  If it's currently
    /// `MigrateData`, moves to `RunNodeAsValidator` using the next installed version.
    fn upgrade_state(&mut self) -> Result<()> {
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
        Ok(())
    }

    /// Sets `self.state` to a new state corresponding to downgrading the current node version.
    ///
    /// Regardless of the current state variant, the returned state is `RunNodeAsValidator` with the
    /// previous installed version.
    fn downgrade_state(&mut self) -> Result<()> {
        let node_info = match &self.state {
            State::RunNodeAsValidator(old_info) => old_info,
            State::MigrateData { new_info, .. } => new_info,
        };

        let previous_version = self.previous_installed_version(&node_info.version)?;
        if previous_version >= node_info.version {
            let msg = format!(
                "no lower version than current {} installed",
                node_info.version
            );
            warn!("{}", msg);
            bail!(msg);
        }

        let new_info = self.new_node_info(previous_version);
        self.state = State::RunNodeAsValidator(new_info);
        Ok(())
    }

    /// Moves the launcher state forward.
    fn transition_state(&mut self, previous_exit_code: NodeExitCode) -> Result<()> {
        match previous_exit_code {
            NodeExitCode::Success => self.upgrade_state()?,
            NodeExitCode::ShouldDowngrade => self.downgrade_state()?,
        }
        self.write()
    }

    /// Runs the process for the current state and moves the state forward if the process exits with
    /// success.
    fn step(&mut self) -> Result<()> {
        let exit_code = match &self.state {
            State::RunNodeAsValidator(node_info) => {
                let mut command = Command::new(&node_info.binary_path);
                command
                    .arg(VALIDATOR_SUBCOMMAND)
                    .arg(&node_info.config_path);
                let exit_code = utils::run_node(command)?;
                info!(version=%node_info.version, "finished running node as validator");
                exit_code
            }
            State::MigrateData { old_info, new_info } => {
                let mut command = Command::new(&new_info.binary_path);
                command
                    .arg(MIGRATE_SUBCOMMAND)
                    .arg(OLD_CONFIG_ARG)
                    .arg(&old_info.config_path)
                    .arg(NEW_CONFIG_ARG)
                    .arg(&new_info.config_path);
                let exit_code = utils::run_node(command)?;
                info!(
                    old_version=%old_info.version,
                    new_version=%new_info.version,
                    "finished data migration"
                );
                exit_code
            }
        };

        self.transition_state(exit_code)
    }
}

#[cfg(test)]
mod tests {
    use std::{os::unix::fs::PermissionsExt, thread, time::Duration};

    use super::*;
    use crate::logging;

    const NODE_CONTENTS: &str = include_str!("../test_resources/casper-node.in");
    const DOWNGRADE_CONTENTS: &str = include_str!("../test_resources/downgrade.in");
    /// The duration to wait after starting a mock casper-node instance before "installing" a new
    /// version of the mock node.  The mock sleeps for 1 second while running in validator mode, so
    /// 100ms should be enough to allow the node-launcher step to start.
    const DELAY_INSTALL_DURATION: Duration = Duration::from_millis(100);
    static V1: Lazy<Version> = Lazy::new(|| Version::new(1, 0, 0));
    static V2: Lazy<Version> = Lazy::new(|| Version::new(2, 0, 0));
    static V3: Lazy<Version> = Lazy::new(|| Version::new(3, 0, 0));

    /// If `upgrade` is true, installs the new version of the mock node binary, assigning an old
    /// version for the script with the major version of `new_version` decremented by 1.
    ///
    /// If `upgrade` is false, installs a copy of the downgrade.sh script in place of the mock node
    /// script.  This script always exits with a code of 102.
    fn install_mock(new_version: &Version, upgrade: bool) {
        if thread::current().name().is_none() {
            panic!(
                "install_mock must be called from the main test thread in order for \
                `Launcher::binary_root_dir()` and `Launcher::config_root_dir()` to work"
            );
        }

        let subdir_name = new_version.to_string().replace(".", "_");

        // Create the node script contents.
        let old_version = Version::new(new_version.major - 1, new_version.minor, new_version.patch);
        let node_contents = if upgrade {
            NODE_CONTENTS
        } else {
            DOWNGRADE_CONTENTS
        };
        let node_contents = node_contents.replace(
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

        install_mock(&*V1, true);
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
        install_mock(&*V1, true);
        let _ = Launcher::new().unwrap();

        // Install a new version of node, but ensure a new launcher reads the state from disk rather
        // than detecting a new version.
        install_mock(&*V2, true);
        let launcher = Launcher::new().unwrap();

        let expected_node_info = launcher.new_node_info(V1.clone());
        let expected_state = State::RunNodeAsValidator(expected_node_info);
        assert_eq!(expected_state, launcher.state);
    }

    #[test]
    fn should_error_if_state_corrupted() {
        let _ = logging::init();

        // Write the state to disk (RunNodeAsValidator for V1).
        install_mock(&*V1, true);
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
    fn should_run_most_recent_version_when_state_file_absent() {
        let _ = logging::init();

        // Set up the test folders as if casper-node has just been staged at v3.0.0,
        // but create the state file, so that the launcher launches the v1.0.0.
        install_mock(&*V1, true);
        install_mock(&*V2, true);
        install_mock(&*V3, true);

        let mut launcher = Launcher::new().unwrap();

        // Run the launcher's first and only step - should run node v3.0.0 in validator mode.  As there
        // will be no further upgraded binary available after the node exits, the step should return
        // an error.
        let error = launcher.step().unwrap_err().to_string();
        assert_last_log_line_eq(&launcher, "Node v3.0.0 ran as validator");
        assert_eq!("no higher version than current 3.0.0 installed", error);
    }

    #[test]
    fn should_run_upgrades() {
        let _ = logging::init();

        // Set up the test folders as if casper-node has just been staged at v3.0.0,
        // but create the state file, so that the launcher launches the v1.0.0.
        install_mock(&*V1, true);
        Launcher::new().unwrap();
        install_mock(&*V2, true);
        install_mock(&*V3, true);

        let mut launcher = Launcher::new().unwrap();
        // Run the launcher's first step - should run node v1.0.0 in validator mode.
        launcher.step().unwrap();
        assert_last_log_line_eq(&launcher, "Node v1.0.0 ran as validator");

        // Run the launcher's second step - should run node v2.0.0 in data-migration mode.
        launcher.step().unwrap();
        assert_last_log_line_eq(&launcher, "Node v2.0.0 migrated data");

        // Run the launcher's third step - should run node v2.0.0 in validator mode.
        launcher.step().unwrap();
        assert_last_log_line_eq(&launcher, "Node v2.0.0 ran as validator");

        // Run the launcher's fourth step - should run node v3.0.0 in data-migration mode.
        launcher.step().unwrap();
        assert_last_log_line_eq(&launcher, "Node v3.0.0 migrated data");

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

        install_mock(&*V2, true);

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
        install_mock(&*V1, true);

        worker.join().unwrap();
    }

    #[test]
    fn should_run_downgrades() {
        let _ = logging::init();

        // Set up the test folders so that v3.0.0 is installed, but it will exit requesting a
        // downgrade.
        install_mock(&*V3, false);

        // Set up a thread to run the launcher.
        let mut launcher = Launcher::new().unwrap();
        let worker = thread::spawn(move || {
            // Run the launcher's first step - should run the downgrader, taking 1 second to
            // complete.
            launcher.step().unwrap();
            assert_last_log_line_eq(&launcher, "Node v3.0.0 exiting to downgrade");
            launcher
        });

        // Install node v2.0.0 also as a downgrader after v3.0.0 has started running.
        thread::sleep(DELAY_INSTALL_DURATION);
        install_mock(&*V2, false);

        launcher = worker.join().unwrap();

        // Set up a thread to run the launcher again.
        let worker = thread::spawn(move || {
            // Run the launcher's second step - should run the downgrader, taking 1 second to
            // complete.
            launcher.step().unwrap();
            assert_last_log_line_eq(&launcher, "Node v2.0.0 exiting to downgrade");
            launcher
        });

        // Install node v2.0.0 also as a downgrader after v3.0.0 has started running.
        thread::sleep(DELAY_INSTALL_DURATION);
        install_mock(&*V1, true);

        launcher = worker.join().unwrap();

        // Run the launcher's third step - should run node v1.0.0 in validator mode.
        launcher.step().unwrap();
        assert_last_log_line_eq(&launcher, "Node v1.0.0 ran as validator");
    }

    #[test]
    fn should_not_downgrade_to_higher_version() {
        let _ = logging::init();

        // Set up the test folders so that v2.0.0 is installed, but it will exit requesting a
        // downgrade.
        install_mock(&*V2, false);

        // Set up a thread to run the launcher.
        let mut launcher = Launcher::new().unwrap();
        let worker = thread::spawn(move || {
            // Run the launcher's first step - should run the downgrader, taking 1 second to
            // complete, but then fail to find an older installed version.
            let error = launcher.step().unwrap_err().to_string();
            assert_last_log_line_eq(&launcher, "Node v2.0.0 exiting to downgrade");
            assert_eq!("no lower version than current 2.0.0 installed", error);
        });

        // Install node v3.0.0 after v2.0.0 has started running.
        thread::sleep(DELAY_INSTALL_DURATION);
        install_mock(&*V3, true);

        worker.join().unwrap();
    }

    #[test]
    fn should_run_again_after_crash_while_in_validator_mode() {
        let _ = logging::init();

        // Set up the test folders as if casper-node has just been installed, but provide a config
        // file which will cause the node to crash as soon as it starts.
        install_mock(&*V1, true);
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
        install_mock(&*V1, true);

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
        install_mock(&*V2, true);
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

        install_mock(&*V1, true);
        install_mock(&*V2, true);
        install_mock(&*V3, true);
        // Rename config folders to emulate the difference.
        fs::rename(
            Launcher::config_root_dir().join("1_0_0"),
            Launcher::config_root_dir().join("2_0_1"),
        )
        .unwrap();

        let error = Launcher::new().unwrap_err().to_string();
        assert_eq!(
            "installed binary versions (1.0.0, 2.0.0, 3.0.0) don't match installed configs (2.0.0, 2.0.1, 3.0.0)",
            error
        );
    }
    #[test]
    fn should_error_if_no_versions_are_installed() {
        let _ = logging::init();

        let error = Launcher::new().unwrap_err().to_string();
        assert!(error.contains("failed to get a valid version from subdirs"));
    }
}

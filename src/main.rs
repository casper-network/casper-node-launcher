#![warn(unused_qualifications)]

mod config;
mod logging;

use std::{fs, path::Path, process::Command};

use anyhow::{bail, Error, Result};
use clap::{crate_description, crate_version, App, Arg};
use tracing::{debug, info, warn};

use config::Config;

const APP_NAME: &str = "Casper node launcher";
const ARG_NAME: &str = "config";

const MIGRATE_SUBCOMMAND: &str = "migrate-data";
const OLD_CONFIG_ARG: &str = "--old-config";
const NEW_CONFIG_ARG: &str = "--new-config";
const VALIDATOR_SUBCOMMAND: &str = "validator";

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum NodeExecutionMode {
    DataMigration,
    Validator,
}

/// Deduces which mode to run the node in by examining the existence or otherwise of the old and new
/// node binaries and their config files.
fn calculate_mode(config: &Config) -> NodeExecutionMode {
    let next_config_exists = config.node_config_next_path().is_file();
    let next_binary_exists = Config::node_binary_next_path().is_file();

    if next_config_exists && !next_binary_exists {
        // A new version has been installed and the node-launcher has already replaced the
        // old binary with the new.  We need to perform data-migration.
        NodeExecutionMode::DataMigration
    } else {
        NodeExecutionMode::Validator
    }
}

fn run_command(mut command: Command) -> Result<()> {
    let mut child = command.spawn().map_err(|error| {
        let msg = format!("failed to execute {:?}", command);
        warn!(%error, "{}", msg);
        Error::new(error).context(msg)
    })?;

    let exit_status = child.wait().map_err(|error| {
        let msg = format!("failed to wait for completion of {:?}", command);
        warn!(%error, "{}", msg);
        Error::new(error).context(msg)
    })?;
    if !exit_status.success() {
        warn!(%exit_status, "failed running {:?}", command);
        bail!("{:?} exited with error", command);
    }

    debug!("successfully finished running {:?}", command);
    Ok(())
}

fn rename(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to).map_err(|error| {
        let msg = format!("failed renaming {} to {}", from.display(), to.display());
        warn!(%error, "{}", msg);
        Error::new(error).context(msg)
    })?;
    debug!("renamed {} to {}", from.display(), to.display());
    Ok(())
}

/// Run the node in `migrate-data` mode.
fn migrate_data(config: &Config) -> Result<()> {
    let old_config_path = config.node_config_path();
    let new_config_path = config.node_config_next_path();

    let mut command = Command::new(Config::node_binary_path());
    command
        .arg(MIGRATE_SUBCOMMAND)
        .arg(OLD_CONFIG_ARG)
        .arg(old_config_path)
        .arg(NEW_CONFIG_ARG)
        .arg(&new_config_path);
    run_command(command)?;

    rename(&new_config_path, old_config_path)?;

    info!("finished data migration");
    Ok(())
}

/// Run the node in `validator` mode.
fn run_as_validator(config: &Config) -> Result<()> {
    let mut command = Command::new(Config::node_binary_path());
    command
        .arg(VALIDATOR_SUBCOMMAND)
        .arg(config.node_config_path());
    run_command(command)?;

    if !Config::node_binary_next_path().is_file() {
        warn!(
            expected_binary=%Config::node_binary_next_path().display(),
            "missing next version of casper-node binary"
        );
        bail!(
            "next casper-node binary doesn't exist at {}",
            Config::node_binary_next_path().display()
        );
    }

    rename(
        &Config::node_binary_next_path(),
        &Config::node_binary_path(),
    )?;

    info!("finished running node as validator");
    Ok(())
}

fn step(config: &Config) -> Result<()> {
    match calculate_mode(&config) {
        NodeExecutionMode::DataMigration => migrate_data(config)?,
        NodeExecutionMode::Validator => run_as_validator(config)?,
    }
    Ok(())
}

fn main() -> Result<()> {
    logging::init()?;

    let app = App::new(APP_NAME)
        .version(crate_version!())
        .about(crate_description!())
        .arg(
            Arg::new(ARG_NAME)
                .value_name("PATH")
                .about("The path to the casper-node config file"),
        );
    let matches = app.get_matches();
    let maybe_node_config_path = matches.value_of(ARG_NAME);

    let config = Config::new(maybe_node_config_path)?;
    debug!("{:?}", config);

    loop {
        step(&config)?;
    }
}

#[cfg(test)]
mod tests {
    use std::{os::unix::fs::PermissionsExt, thread, time::Duration};

    use super::*;

    const NODE_CONTENTS: &str = include_str!("../test_resources/casper-node.in");
    /// The duration to wait after starting a mock casper-node instance before "installing" a new
    /// version of the mock node.  The mock sleeps for 1 second while running in validator mode, so
    /// 100ms should be enough to allow the node-launcher step to start.
    const DELAY_INSTALL_DURATION: Duration = Duration::from_millis(100);

    fn create_executable_file(path: &Path, contents: &str) {
        fs::write(path, contents.as_bytes()).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    /// Sets the values for `VERSION` and `OLD_VERSION` in the mock casper-node script, and creates
    /// the mock binary at the given path.
    fn create_mock_node_binary(path: &Path, old_version: &str, new_version: &str) {
        let node_contents = NODE_CONTENTS.replace(
            r#"OLD_VERSION="""#,
            &format!(r#"OLD_VERSION="{}""#, old_version),
        );
        let node_contents =
            node_contents.replace(r#"VERSION="""#, &format!(r#"VERSION="{}""#, new_version));
        create_executable_file(path, &node_contents);
    }

    /// Asserts that `line` equals the last line in the log file which the mock casper-node should
    /// have written to.
    fn assert_last_log_line_eq(line: &str) {
        let log_path = Config::node_binary_path().parent().unwrap().join("log.txt");
        let log_contents = fs::read_to_string(&log_path).unwrap();
        assert_eq!(line, log_contents.lines().last().unwrap());
    }

    /// Asserts that the last line in the log file which the mock casper-node should have written to
    /// contains the provided string.
    fn assert_last_log_line_contains(string: &str) {
        let log_path = Config::node_binary_path().parent().unwrap().join("log.txt");
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
    fn should_run_upgrades() {
        let _ = logging::init();

        // Set up the test folders as if casper-node has just been installed.
        create_mock_node_binary(&Config::node_binary_path(), "0.0.1", "1.0.0");
        fs::write(Config::default_node_config_path(), "1.0.0".as_bytes()).unwrap();

        // Set up a thread to run while the node-launcher is running, "installing" a new node binary
        // and config after the first has started running.
        let node_next_path = Config::node_binary_next_path();
        let config = Config::new(None).unwrap();
        let node_config_next_path = config.node_config_next_path();

        let worker = thread::spawn(move || {
            thread::sleep(DELAY_INSTALL_DURATION);
            create_mock_node_binary(&node_next_path, "1.0.0", "2.0.0");
            fs::write(&node_config_next_path, "2.0.0".as_bytes()).unwrap();
        });

        // Run the node-launcher's first step - should run node v1.0.0 in validator mode.
        step(&config).unwrap();
        assert_last_log_line_eq("Node v1.0.0 ran as validator");
        assert!(
            !Config::node_binary_next_path().exists(),
            "v2 binary should not still exist"
        );
        assert!(
            config.node_config_next_path().exists(),
            "v2 config should still exist"
        );

        // Run the node-launcher's second step - should run node v2.0.0 in data-migration mode.
        step(&config).unwrap();
        worker.join().unwrap();
        assert_last_log_line_eq("Node v2.0.0 migrated data");
        assert!(
            !config.node_config_next_path().exists(),
            "v2 config should not still exist"
        );

        // Set up a thread to run while the node-launcher is running, "installing" another new node
        // binary and config after the second has started running.
        let node_next_path = Config::node_binary_next_path();
        let node_config_next_path = config.node_config_next_path();
        let worker = thread::spawn(move || {
            thread::sleep(DELAY_INSTALL_DURATION);
            create_mock_node_binary(&node_next_path, "2.0.0", "3.0.0");
            fs::write(&node_config_next_path, "3.0.0".as_bytes()).unwrap();
        });

        // Run the node-launcher's third step - should run node v2.0.0 in validator mode.
        step(&config).unwrap();
        assert_last_log_line_eq("Node v2.0.0 ran as validator");
        assert!(
            !Config::node_binary_next_path().exists(),
            "v3 binary should not still exist"
        );
        assert!(
            config.node_config_next_path().exists(),
            "v3 config should still exist"
        );

        // Run the node-launcher's fourth step - should run node v3.0.0 in data-migration mode.
        step(&config).unwrap();
        worker.join().unwrap();
        assert_last_log_line_eq("Node v3.0.0 migrated data");
        assert!(
            !config.node_config_next_path().exists(),
            "v3 config should not still exist"
        );

        // Run the node-launcher's fifth step - should run node v3.0.0 in validator mode.  As there
        // will be no further upgraded binary available after the node exits, the step should return
        // an error.
        let error = step(&config).unwrap_err();
        assert_last_log_line_eq("Node v3.0.0 ran as validator");
        let error_msg = error.downcast_ref::<String>().unwrap();
        assert!(
            error_msg.starts_with("next casper-node binary doesn't exist at"),
            "{}",
            error_msg
        );
    }

    #[test]
    fn should_run_again_after_crash_while_in_validator_mode() {
        let _ = logging::init();

        // Set up the test folders as if casper-node has just been installed, but provide a config
        // file which will cause the node to crash as soon as it starts.
        create_mock_node_binary(&Config::node_binary_path(), "0.0.1", "1.0.0");
        fs::write(Config::default_node_config_path(), "bad value".as_bytes()).unwrap();

        // Run the node-launcher step - should return an error indicating the node exited with an
        // error, but should have left the node and config unchanged and still runnable.
        let config = Config::new(None).unwrap();
        let error = step(&config).unwrap_err();
        assert_last_log_line_contains("should contain 1.0.0 but contains bad value");
        let error_msg = error.downcast_ref::<String>().unwrap();
        assert!(error_msg.contains("exited with error"), "{}", error_msg);
        assert!(
            Config::node_binary_path().exists(),
            "v1 binary should still exist"
        );
        assert!(
            config.node_config_path().exists(),
            "v1 config should still exist"
        );

        // Fix the config file to be valid and try running the node again.  The node-launcher will
        // error out again, but this time after the node has finished running in validator mode due
        // to there being no upgraded binary available after the node exits.
        fs::write(Config::default_node_config_path(), "1.0.0".as_bytes()).unwrap();
        let error = step(&config).unwrap_err();
        assert_last_log_line_eq("Node v1.0.0 ran as validator");
        let error_msg = error.downcast_ref::<String>().unwrap();
        assert!(
            error_msg.starts_with("next casper-node binary doesn't exist at"),
            "{}",
            error_msg
        );
    }

    #[test]
    fn should_run_again_after_crash_while_in_migration_mode() {
        let _ = logging::init();

        // Set up the test folders as if casper-node has just been installed.
        create_mock_node_binary(&Config::node_binary_path(), "0.0.1", "1.0.0");
        fs::write(Config::default_node_config_path(), "1.0.0".as_bytes()).unwrap();

        // Set up a thread to run while the node-launcher is running, "installing" a new node binary
        // and config after the first has started running, but provide a config file which will
        // cause the node to crash as soon as it starts.
        let node_next_path = Config::node_binary_next_path();
        let config = Config::new(None).unwrap();
        let node_config_next_path = config.node_config_next_path();

        let worker = thread::spawn(move || {
            thread::sleep(DELAY_INSTALL_DURATION);
            create_mock_node_binary(&node_next_path, "1.0.0", "2.0.0");
            fs::write(&node_config_next_path, "bad value".as_bytes()).unwrap();
        });

        // Run the node-launcher step - should run node v1.0.0 in validator mode.
        step(&config).unwrap();
        assert_last_log_line_eq("Node v1.0.0 ran as validator");

        // Run the node-launcher step - should run node v2.0.0 in data-migration mode but should
        // return an error indicating the node exited with an error, and should have left the node
        // and two configs unchanged and still runnable.
        let error = step(&config).unwrap_err();
        worker.join().unwrap();
        assert_last_log_line_contains("should contain 2.0.0 but contains bad value");
        let error_msg = error.downcast_ref::<String>().unwrap();
        assert!(error_msg.contains("exited with error"), "{}", error_msg);
        assert!(
            Config::node_binary_path().exists(),
            "v2 binary should still exist"
        );
        assert!(
            config.node_config_path().exists(),
            "v1 config should still exist"
        );
        assert!(
            config.node_config_next_path().exists(),
            "v2 config should still exist"
        );

        // Fix the config file to be valid and try running the node again.  It should run in data-
        // migration mode again.
        fs::write(config.node_config_next_path(), "2.0.0".as_bytes()).unwrap();
        step(&config).unwrap();
        assert_last_log_line_eq("Node v2.0.0 migrated data");
        assert!(
            Config::node_binary_path().exists(),
            "v2 binary should still exist"
        );
        assert!(config.node_config_path().exists(), "v2 config should exist");
        assert!(
            !config.node_config_next_path().exists(),
            "v2 config should have been renamed"
        );
    }
}

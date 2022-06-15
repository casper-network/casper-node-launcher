use std::{
    collections::BTreeSet, fmt::Display, fs, path::Path, process::Command, str::FromStr,
    sync::atomic::Ordering,
};

use anyhow::{bail, Error, Result};
use semver::Version;
use tracing::{debug, warn};

/// Represents the exit code of the node process.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(i32)]
pub(crate) enum NodeExitCode {
    /// Indicates a successful execution.
    Success = 0,
    /// Indicates the node version should be downgraded.
    ShouldDowngrade = 102,
    /// Indicates the node launcher should attempt to run the shutdown script.
    ShouldExitLauncher = 103,
}

/// Iterates the given path, returning the subdir representing the immediate next SemVer version
/// after `current_version`.
///
/// Subdir names should be semvers with dots replaced with underscores.
pub(crate) fn next_installed_version<P: AsRef<Path>>(
    dir: P,
    current_version: &Version,
) -> Result<Version> {
    let max_version = Version::new(u64::max_value(), u64::max_value(), u64::max_value());

    let mut next_version = max_version.clone();
    for installed_version in versions_from_path(dir)? {
        if installed_version > *current_version && installed_version < next_version {
            next_version = installed_version;
        }
    }

    if next_version == max_version {
        next_version = current_version.clone();
    }

    Ok(next_version)
}

/// Iterates the given path, returning the subdir representing the immediate previous SemVer version
/// before `current_version`.
///
/// Subdir names should be semvers with dots replaced with underscores.
pub(crate) fn previous_installed_version<P: AsRef<Path>>(
    dir: P,
    current_version: &Version,
) -> Result<Version> {
    let min_version = Version::new(0, 0, 0);

    let mut previous_version = min_version.clone();
    for installed_version in versions_from_path(dir)? {
        if installed_version < *current_version && installed_version > previous_version {
            previous_version = installed_version;
        }
    }

    if previous_version == min_version {
        previous_version = current_version.clone();
    }

    Ok(previous_version)
}

pub(crate) fn versions_from_path<P: AsRef<Path>>(dir: P) -> Result<BTreeSet<Version>> {
    let mut versions = BTreeSet::new();

    for entry in map_and_log_error(
        fs::read_dir(dir.as_ref()),
        format!("failed to read dir {}", dir.as_ref().display()),
    )? {
        let path = map_and_log_error(
            entry,
            format!("bad dir entry in {}", dir.as_ref().display()),
        )?
        .path();
        let subdir_name = match path.file_name() {
            Some(name) => name.to_string_lossy().replace('_', "."),
            None => {
                debug!("{} has no final path component", path.display());
                continue;
            }
        };
        let version = match Version::from_str(&subdir_name) {
            Ok(version) => version,
            Err(error) => {
                debug!(%error, path=%path.display(), "failed to get a version");
                continue;
            }
        };

        versions.insert(version);
    }

    if versions.is_empty() {
        let msg = format!(
            "failed to get a valid version from subdirs in {}",
            dir.as_ref().display()
        );
        warn!("{}", msg);
        bail!(msg);
    }

    Ok(versions)
}

/// Runs the given command as a child process.
pub(crate) fn run_node(mut command: Command) -> Result<NodeExitCode> {
    let mut child = map_and_log_error(command.spawn(), format!("failed to execute {:?}", command))?;
    crate::CHILD_PID.store(child.id(), Ordering::SeqCst);

    let exit_status = map_and_log_error(
        child.wait(),
        format!("failed to wait for completion of {:?}", command),
    )?;
    match exit_status.code() {
        Some(code) if code == NodeExitCode::Success as i32 => {
            debug!("successfully finished running {:?}", command);
            Ok(NodeExitCode::Success)
        }
        Some(code) if code == NodeExitCode::ShouldDowngrade as i32 => {
            debug!("finished running {:?} - should downgrade now", command);
            Ok(NodeExitCode::ShouldDowngrade)
        }
        Some(code) if code == NodeExitCode::ShouldExitLauncher as i32 => {
            debug!(
                "finished running {:?} - trying to run shutdown script now",
                command
            );
            Ok(NodeExitCode::ShouldExitLauncher)
        }
        _ => {
            warn!(%exit_status, "failed running {:?}", command);
            bail!("{:?} exited with error", command);
        }
    }
}

/// Maps an error to a different type of error, while also logging the error at warn level.
pub(crate) fn map_and_log_error<T, E: std::error::Error + Send + Sync + 'static>(
    result: std::result::Result<T, E>,
    error_msg: String,
) -> Result<T> {
    match result {
        Ok(t) => Ok(t),
        Err(error) => {
            warn!(%error, "{}", error_msg);
            Err(Error::new(error).context(error_msg))
        }
    }
}

/// Joins the items into a single string.
/// The input `[1, 2, 3]` will result in a string "1, 2, 3".
pub(crate) fn iter_to_string<I>(iterable: I) -> String
where
    I: IntoIterator,
    I::Item: Display,
{
    // This function should ideally be replaced with `itertools::join()`.
    // However, currently, it is only used to produce a proper debug message,
    // which is not sufficient justification to add a dependency to `itertools`.
    let result = iterable.into_iter().fold(String::new(), |result, item| {
        format!("{}{}, ", result, item)
    });
    if result.is_empty() {
        result
    } else {
        String::from(&result[0..result.len() - 2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging;

    #[test]
    fn should_get_next_installed_version() {
        let _ = logging::init();
        let tempdir = tempfile::tempdir().expect("should create temp dir");

        let get_next_version = |current_version: &Version| {
            next_installed_version(tempdir.path(), current_version).unwrap()
        };

        let mut current = Version::new(0, 0, 0);
        let mut next_version = Version::new(1, 0, 0);
        fs::create_dir(tempdir.path().join("1_0_0")).unwrap();
        assert_eq!(get_next_version(&current), next_version);
        current = next_version;

        next_version = Version::new(1, 2, 3);
        fs::create_dir(tempdir.path().join("1_2_3")).unwrap();
        assert_eq!(get_next_version(&current), next_version);
        current = next_version.clone();

        fs::create_dir(tempdir.path().join("1_0_3")).unwrap();
        assert_eq!(get_next_version(&current), next_version);

        fs::create_dir(tempdir.path().join("2_2_2")).unwrap();
        fs::create_dir(tempdir.path().join("3_3_3")).unwrap();
        assert_eq!(get_next_version(&current), Version::new(2, 2, 2));
    }

    #[test]
    fn should_ignore_invalid_versions() {
        let _ = logging::init();
        let tempdir = tempfile::tempdir().expect("should create temp dir");
        let current_version = Version::new(0, 0, 0);

        // Try with a non-existent dir.
        let non_existent_dir = Path::new("not_a_dir");
        let error = next_installed_version(non_existent_dir, &current_version)
            .unwrap_err()
            .to_string();
        assert_eq!(
            format!("failed to read dir {}", non_existent_dir.display()),
            error
        );

        // Try with a dir which has no subdirs.
        let error = next_installed_version(tempdir.path(), &current_version)
            .unwrap_err()
            .to_string();
        assert_eq!(
            format!(
                "failed to get a valid version from subdirs in {}",
                tempdir.path().display()
            ),
            error
        );

        // Try with a dir which has one subdir which is not a valid version representation.
        fs::create_dir(tempdir.path().join("not_a_version")).unwrap();
        let error = next_installed_version(tempdir.path(), &current_version)
            .unwrap_err()
            .to_string();
        assert_eq!(
            format!(
                "failed to get a valid version from subdirs in {}",
                tempdir.path().display()
            ),
            error
        );

        // Try with a dir which has a valid and invalid subdir - the invalid one should be ignored.
        fs::create_dir(tempdir.path().join("1_2_3")).unwrap();
        assert_eq!(
            next_installed_version(tempdir.path(), &current_version).unwrap(),
            Version::new(1, 2, 3)
        );
    }

    #[test]
    fn should_not_run_invalid_command() {
        let _ = logging::init();

        // Try with a non-existent binary.
        let non_existent_binary = "non-existent-binary";
        let mut command = Command::new(non_existent_binary);
        let error = run_node(command).unwrap_err().to_string();
        assert_eq!(
            format!(r#"failed to execute "{}""#, non_existent_binary),
            error
        );

        // Try a valid binary but use a bad arg to make it exit with a failure.
        let cargo = env!("CARGO");
        command = Command::new(cargo);
        command.arg("--deliberately-passing-bad-arg-for-test");
        let error = run_node(command).unwrap_err().to_string();
        assert!(error.ends_with("exited with error"), "{}", error);
    }

    #[test]
    fn should_run_valid_command() {
        let _ = logging::init();

        let cargo = env!("CARGO");
        let mut command = Command::new(cargo);
        command.arg("--version");
        assert_eq!(run_node(command).unwrap(), NodeExitCode::Success);
    }

    #[test]
    fn should_run_command_exiting_with_downgrade_code() {
        let _ = logging::init();

        let script_path = format!("{}/test_resources/downgrade.sh", env!("CARGO_MANIFEST_DIR"));

        let mut command = Command::new("sh");
        command.arg(&script_path);
        assert_eq!(run_node(command).unwrap(), NodeExitCode::ShouldDowngrade);
    }

    #[test]
    fn should_read_versions_from_dir() {
        let tempdir = tempfile::tempdir().expect("should create temp dir");
        fs::create_dir(tempdir.path().join("1_0_0")).unwrap();
        fs::create_dir(tempdir.path().join("2_0_0")).unwrap();
        fs::create_dir(tempdir.path().join("3_0_0")).unwrap();
        fs::create_dir(tempdir.path().join("3_0_0_0")).unwrap();
        fs::create_dir(tempdir.path().join("3_A")).unwrap();
        fs::create_dir(tempdir.path().join("2_0_1")).unwrap();
        fs::create_dir(tempdir.path().join("1_0_9145")).unwrap();
        fs::create_dir(tempdir.path().join("1_454875135649876544411657897987_9145")).unwrap();

        // Should return in ascending order
        let expected_version: BTreeSet<Version> = [
            Version::new(1, 0, 0),
            Version::new(1, 0, 9145),
            Version::new(2, 0, 0),
            Version::new(2, 0, 1),
            Version::new(3, 0, 0),
        ]
        .iter()
        .cloned()
        .collect();

        let actual_versions = versions_from_path(tempdir.path()).unwrap();

        assert_eq!(expected_version, actual_versions);
    }

    #[test]
    fn concatenates_iterable_values() {
        let input = ["abc"];
        let output = iter_to_string(input);
        assert_eq!(output, "abc");

        let input = ["a", "b", "c"];
        let output = iter_to_string(input);
        assert_eq!(output, "a, b, c");

        let input = Vec::<i32>::new();
        let output = iter_to_string(input);
        assert_eq!(output, "");
    }
}

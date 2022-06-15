# Changelog

All notable changes to this project will be documented in this file.  The format is based on [Keep a Changelog].

[comment]: <> (Added:      new features)
[comment]: <> (Changed:    changes in existing functionality)
[comment]: <> (Deprecated: soon-to-be removed features)
[comment]: <> (Removed:    now removed features)
[comment]: <> (Fixed:      any bug fixes)
[comment]: <> (Security:   in case of vulnerabilities)

## [Unreleased]
### Added
* Launcher now handles node exit code `103` by running a script at `/etc/casper/casper_shutdown_script` and exiting with its exit code if present, otherwise returning 0.

## [1.0.0] - 2022-01-10

### Added
* Commented out line provided in systemd unit to allow users to set hard limit of files to 64000 (from default 4096).
* node_util.py updates to expand capability
* Deprecation warning to older scripts
* README.md updates related to configuration of nofile limit

## [0.3.5] - 2021-10-25

### Added
* node_util.py script to gradually replace various shell scripts in /etc/casper
* BIN_MODE to network configs

### Removed
* Docker image build and publish
* bintray deb publish

## [0.3.4] - 2021-07-27

### Added
* RPM package build
* Publish DEB and RPM package to GitHub tag
* PLATFORM file install to indicate system type

### Changed
* License from COSL to Apache

## [0.3.3] - 2021-04-06

### Added
* Network configurations to allow pulling protocol versions from a configurable location
* Verification of running under casper user for scripts
* Improvement of external IP detection for config_from_example.sh
* Network configurations for casper and casper-test networks

## [0.3.2] - 2021-03-19

### Changed
* Package install README updates
* Better validation of pull_casper_node_version.sh

### Removed
* systemd environment arg for legacy net

## [0.3.1] - 2021-03-10

### Added
* Docker image build capability
* Better validation to pull_casper_node_version.sh

### Changed
* systemd unit restart time limit set to 15 seconds

## [0.3.0] - 2021-02-17

### Added
* 3 start retry within 1000 seconds and 1 sec restart delay to systemd unit
* copytruncate to logrotate
* Downgrade capability

## 0.2.0 - 2021-02-08

Initial Public Release

[Keep a Changelog]: https://keepachangelog.com/en/1.0.0
[unreleased]: https://github.com/casper-network/casper-node-launcher/compare/v0.4.0...main
[1.0.0]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.5...v1.0.0
[0.3.5]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/casper-network/casper-node-launcher/compare/v0.2.0...v0.3.0

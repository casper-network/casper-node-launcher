# Changelog

All notable changes to this project will be documented in this file.  The format is based on [Keep a Changelog].

[comment]: <> (Added:      new features)
[comment]: <> (Changed:    changes in existing functionality)
[comment]: <> (Deprecated: soon-to-be removed features)
[comment]: <> (Removed:    now removed features)
[comment]: <> (Fixed:      any bug fixes)
[comment]: <> (Security:   in case of vulnerabilities)

## [Unreleased]

### Changed
* Upgraded `clap` from 3 to 4, to drop the unmaintained `atty` dependency. This clears RUSTSEC-2024-0375 and RUSTSEC-2021-0145.

## [1.0.8] - 2025-07-30

### Added
* `rust-toolchain.toml` pinning the Rust toolchain to 1.85.1, matching the version used by casper-node.

### Changed
* CI runners moved from Ubuntu 20.04 to 22.04.
* `publish_deb_to_repo.sh` now creates the aptly mirror with a filter excluding `casper-node-launcher` packages.
* Clippy and rustfmt fixes across `launcher.rs`, `main.rs` and `utils.rs`.

### Removed
* Ubuntu 24.04 (noble) from the build matrix, until the OS name is included in the debian package filename.

## [1.0.7] - 2025-07-24

*Untagged release.*

### Added
* Ubuntu 22.04 (jammy) and 24.04 (noble) deb build targets.

### Changed
* The deb package now depends on `casper-node-util` in addition to `curl`.

### Removed
* Ubuntu 20.04 (focal) deb build target.

## [1.0.6] - 2025-03-17

### Added
* `node_util.py` commands for casper-sidecar: `sidecar_status`, `sidecar_start`, `sidecar_stop`, `sidecar_restart` and `sidecar_log`.
* `node_util.py` commands `node_log` and `node_error_log`.

### Changed
* systemd unit `Documentation` URL updated from `docs.casperlabs.io` to `docs.casper.network`.
* `ETC_README.md` and the network config README updated for current genesis staging and documentation locations.

## [1.0.5] - 2025-03-10

### Changed
* Network configs for casper and casper-test now point at `genesis.casper.network`.
* PR workflow reworked to stop running duplicate jobs.
* Publish workflow AWS permissions corrected, and `publish_deb_to_repo.sh` updated for the new config.
* `node_util.py` and logrotate configuration updates.

### Removed
* RPM packaging, including the `.rpm` spec and the `PLATFORM_RPM` / `PLATFORM_DEB` marker files.
* `config_from_example.sh` and `pull_casper_node_version.sh`, both superseded by `node_util.py`.

## [1.0.4] - 2023-09-01

### Added
* `get_ip` command to `node_util.py`, to show the external IP that would be used to populate `config.toml`.

### Fixed
* External IP detection returning an IPv6 address on dual-stack hosts. Detection now uses IPv4-only endpoints (`4.icanhazip.com`, `4.ident.me`) and validates the result as an IPv4 address.

## [1.0.3] - 2023-08-04

### Changed
* `Cargo.toml` reformatted to accommodate changes in `cargo-deb`.

## [1.0.2] - 2023-08-03

### Added
* GitHub Actions workflows for PR validation and publishing.
* `ci/publish_deb_to_repo.sh`.
* Per-distribution deb variants.

### Removed
* Drone CI (`.drone.yml`) and bors (`bors.toml`).

## [1.0.1] - 2023-07-21

*Untagged release.*

### Added
* Launcher now handles node exit code `103` by running a script at `/etc/casper/casper_shutdown_script` and exiting with its exit code if present, otherwise returning 0.
* `node_util.py` commands `unstage_protocol` and `shift_ports`.
* `size 1G` to the logrotate configuration, so logs rotate on size as well as age.

### Changed
* Exit code `254` is now reported when the shutdown script is terminated by a signal.
* Log messages now start with a lowercase letter.

### Removed
* Unused `shutdown.sh`.

### Security
* Dependency updates to resolve `cargo audit` findings.

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
[unreleased]: https://github.com/casper-network/casper-node-launcher/compare/v1.0.8...main
[1.0.8]: https://github.com/casper-network/casper-node-launcher/compare/v1.0.6...v1.0.8
[1.0.6]: https://github.com/casper-network/casper-node-launcher/compare/v1.0.5...v1.0.6
[1.0.5]: https://github.com/casper-network/casper-node-launcher/compare/v1.0.4...v1.0.5
[1.0.4]: https://github.com/casper-network/casper-node-launcher/compare/v1.0.3...v1.0.4
[1.0.3]: https://github.com/casper-network/casper-node-launcher/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/casper-network/casper-node-launcher/compare/v1.0.0...v1.0.2
[1.0.0]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.5...v1.0.0
[0.3.5]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/casper-network/casper-node-launcher/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/casper-network/casper-node-launcher/compare/v0.2.0...v0.3.0

# casper-node-launcher

A binary which runs and upgrades the casper-node of the Casper network.

## Usage

The casper-node-launcher takes no arguments other than the standard `--help` and `--version`.

On startup, the launcher either tries to read its previously cached state from disk, or assumes a fresh start.  On a
fresh start, the launcher searches for the lowest installed version of `casper-node` and starts running it in validator
mode.

After every run of the `casper-node` binary in validator mode, the launcher does the following based upon the exit code
returned by `casper-node`:
  * If 0 (success), searches for the immediate next installed version of `casper-node` and runs it in migrate-data mode
  * If 102 (downgrade), searches for the immediate previous installed version of `casper-node` and runs it in validator
    mode
  * Any other value causes the launcher to exit with an error

After every run of the `casper-node` binary in migrate-data mode, the launcher does the following based upon the exit
code returned by `casper-node`:
  * If 0 (success), runs the same version of `casper-node` in validator mode
  * If 102 (downgrade), searches for the immediate previous installed version of `casper-node` and runs it in validator
    mode
  * Any other value causes the launcher to exit with an error

If the launcher cannot find an appropriate version at any stage of upgrading or downgrading, it exits with an error.

The default path for the casper-node's config file is `/etc/casper/1_0_0/config.toml` where the folder `1_0_0`
indicates the semver version of the node software.

The default path for the launcher's cached state file is `/etc/casper/casper-node-launcher-state.toml`.

For testing purposes, the common folder `/etc/casper` can be overridden by setting the environment variable
`CASPER_CONFIG_DIR` to a different folder.

The default path for the casper-node binary is `/var/lib/casper/bin/1_0_0/casper-node` where the folder `1_0_0` likewise
indicates the version.  The default path for the casper-node-launcher binary is
`/var/lib/casper/bin/casper-node-launcher`.

For testing purposes, the common folder `/var/lib/casper/bin` can be overridden by setting the environment variable
`CASPER_BIN_DIR` to a different folder.

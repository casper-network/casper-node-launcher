# casper-node-launcher

A binary which runs and upgrades the casper-node of the Casper network.

## Usage

The casper-node-launcher takes no arguments other than the standard `--help` and `--version`.

On startup, the launcher either tries to read its previously cached state from disk, or assumes a fresh start.  On a
fresh start, the launcher searches for the latest installed version of `casper-node` and starts running it in validator
mode.

After every successful run of the `casper-node` binary in validator mode, the launcher again searches for the latest
installed version of casper-node.  If it cannot find a newer version, it exits.  Otherwise it runs the `casper-node` in
migrate-data mode.

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

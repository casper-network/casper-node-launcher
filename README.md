# casper-node-launcher

A binary which runs and upgrades the casper-node of the Casper network.

## Usage

The casper-node-launcher only takes a single optional argument: the path to the casper-node's config file.

This path is cached in the casper-node-launcher's own config file.  On startup, the cache is either read, or written to
if casper-node-launcher is not passed the arg.

If the casper-node's config file doesn't exist, the casper-node-launcher exits with an error.

The default path for the casper-node's config file is `/etc/casper/config.toml`.  The default path for the
casper-node-launcher's config file is `/etc/casper/casper-node-launcher-config.toml`.

For testing purposes, the common folder `/etc/casper` can be overridden by setting the environment variable
`CASPER_CONFIG_DIR` to a different folder.

The default path for the casper-node binary is `/var/lib/casper/bin/casper-node`.  The default path for the
casper-node-launcher binary is `/var/lib/casper/bin/casper-node-launcher`.

For testing purposes, the common folder `/var/lib/casper/bin` can be overridden by setting the environment variable
`CASPER_BIN_DIR` to a different folder.

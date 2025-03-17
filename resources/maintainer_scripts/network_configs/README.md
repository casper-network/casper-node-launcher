# Network Configurations

Network configurations should be of the format:
```
SOURCE_URL=<url for packages>
NETWORK_NAME=<name of network>
```
It is recommended that network_name used is the same as the <network_name>.conf.  This will be executed as 
`source "$DIR/network_configs/<config_file>.conf` to load these variables.

## Usage 

These configurations will be sent to `node_util.py stage_protocols` as an argument.

The target URL is expected to serve HTTP access to `<url>/<network_name>/<protocol_version>/[bin.tar.gz|config.tar.gz]`
Protocol versions to install is expected to exist in `<url>/<network_name>/protocol_versions` as a protocol version per line.

Example:
`sudo -u casper /etc/casper/node_util.py stage_protocols casper.conf`

With `casper.conf` of:
```
SOURCE_URL=genesis.casper.network
NETWORK_NAME=casper
```

Will perform:

Pull and parsing of `genesis.casper.network/casper/protocol_versions`.
Then pulling and installing each protocol listed.

If `protocol_versions` had:

```
2_0_0
```

This would download:
```
curl -JLO genesis.casper.network/casper/2_0_0/bin.tar.gz
curl -JLO genesis.casper.network/casper/2_0_0/config.tar.gz
```
`config.tar.gz` is decompressed into `/etc/casper/<protocol_version>`.
`bin.tar.gz` is decompressed into `/var/lib/casper/<protocol_version>`.

Then `/etc/casper/2_0_0/config.toml` is made from `config-example.toml` in the same directory.

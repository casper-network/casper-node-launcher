# Network Configurations

Network configurations should be of the format:
```
SOURCE_URL=<url for packages>
NETWORK_NAME=<name of network>
```
It is recommended that network_name used is the same as the <network_name>.conf.  This will be executed as 
`source "$DIR/network_configs/<config_file>.conf` to load these variables.

## Usage 

These configurations will be sent to `pull_casper_node_version.sh` as an argument.

The target URL is expected to serve HTTP access to `<url>/<network_name>/<protocol_version>/[bin.tar.gz|config.tar.gz]`

Example:
`sudo -u casper pull_casper_node_version.sh casper.conf 1_0_0`

With `casper.conf` of:
```
SOURCE_URL=genesis.casperlabs.io
NETWORK_NAME=casper
```

Will perform:
```
curl -JLO genesis.casperlabs.io/casper/1_0_0/bin.tar.gz
curl -JLO genesis.casperlabs.io/casper/1_0_0/config.tar.gz
```

`config.tar.gz` is decompressed into `/etc/casper/<protocol_version>`.
`bin.tar.gz` is decompressed into `/var/lib/casper/<protocol_version>`.

The script will error if protocol versions already exist.

## Packaging

With merges to `master`, `release-*` and `dev` branches in the `casper-node` repo, the artifacts are created in
`genesis.casperlabs.io/drone/<git_hash>/<protocol varsion>/[bin.tar.gz|config.tar.gz]`.

You may also pull down the artifacts for a given network and modify to stage a new network.  
If you want to launch a network with the same software version of `casper`, you could pull down the `bin.tar.gz` and 
use as is.  However, your `config.tar.gz` would need modified.

```
mkdir config
curl -JLO genesis.casperlabs.io/casper/1_0_0/bin.tar.gz
curl -JLO genesis.casperlabs.io/casper/1_0_0/config.tar.gz
mv config.tar.gz config_old.tar.gz
cd config
tar -xzvf ../config_old.tar.gz
```

You would need to customize `chainspec.toml` with a new network name and activation_point timestamp.
You would need to customize `config-example.toml" with new known_addresses.

Once all the configuration changes are done.  Create a new config.tar.gz from within the config directory.

```
tar -czvf ../config.tar.gz .
```

Now upload `bin.tar.gz` and `config.tar.gz` to your `<url>/<network>/<protocol_version>` location.
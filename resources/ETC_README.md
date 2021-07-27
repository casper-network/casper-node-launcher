# casper-node-launcher

This package runs the casper-node software and handles changing execution to newer versions at
determined times based on configuration.  This allows simultaneous upgrading of all nodes on the
network.

Please refer to http://docs.casperlabs.io for information on how to run a node.

## systemd

The deb package installs casper-node-launcher service unit for systemd.  If you are unfamiliar with systemd,
the [Arch Linux page on systemd](https://wiki.archlinux.org/index.php/systemd) is a good intro into using it.

Start the casper-node-launcher with:

`sudo systemctl start casper-node-launcher`

Show status of our system:

`systemctl status casper-node-launcher`

### Reading logs

Logs are created in `/var/log/casper/casper-node.log`.

Log rotation is setup in `/etc/logrotate.d/casper-node`.

Logs can be viewed with `sudo cat /var/log/casper/casper-node.log`.  

The logs are in 'json' format.

### Crash logs

Teardown crash logs are created in '/var/log/casper/casper-node.stderr.log'.

These use the same log rotation as `casper-node.log`.

Crash logs can be viewed with `sudo cat /var/log/casper/casper-node.strerr.log`.


### Starting and stopping services

To start service:

`sudo systemctl start casper-node-launcher`

To stop:

`sudo systemctl stop casper-node-launcher`

## Local Storage

If you need to delete the db for a new run,
you can use the script in `/etc/casper` with `sudo /etc/casper/delete_local_db.sh`.

## Staging casper-node protocols

Upgrading is done by staging a new casper-node and configuration prior to the agreed upgrade era.

To simplify this, the `sudo -u casper /etc/casper/pull_casper_node_version.sh [network config] [semver]` script is included.  This will
pull files from the appropriate version.  If desired, the casper-node can be built from source at the 
same version.

To get a working default config.toml for a protocol version: `sudo -u casper /etc/casper/config_from_example.sh [semver]`.


When the upgrade era occurs, the currently running casper-node will exit and casper-node-launcher will
start the new upgraded version of casper-node.

## Bugs

Please file any bugs as issues on the launcher at [the casper-node-launcher GitHub repo](https://github.com/CasperLabs/casper-node-launcher).
Please file any bugs as issues onthe node at [the casper-node GitHub repo](https://github.com/CasperLabs/casper-node).

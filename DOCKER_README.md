# Container for running casper-node-launcher

Scripts that would be in the `/etc/casper` directory with a normal debian install exist with the
`casper-node-launcher` executable.

It is expected that users will mount two volumes to the inside of the container at:
`/etc/casper` and `/var/lib/casper`.

It is also expected that `/etc/casper/validator_keys` exists and is populated by keys from `casper-client keygen`.

For minimum node functionality port 35000 needs to be accessible.  For full functionality, ports 7777, 8888, and 9999
should also be exposed, unless they have been changed in the config.toml.

Initial configuration will consist of calling:
`/root/pull_casper_node_version.sh <protocol version> <network name>`
`/root/config_from_example.sh <protocol version>`

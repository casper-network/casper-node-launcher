# casper-node-launcher

A binary which runs and upgrades the casper-node of the Casper network.

## Usage

```
    casper-node-launcher [OPTIONS]

OPTIONS:
    -f, --force-version <version>    Forces the launcher to run the specified version of the node,
                                     for example "1.2.3"
    -h, --help                       Print help information
    -V, --version                    Print version information
```

On startup, launcher checks whether the installed node binaries match the installed configs,
by comparing the version numbers.  If not, it exits with an error.

The launcher then checks if the `--force-version` parameter was provided.  If yes, it will unconditionally
run the specified node, given it is installed.  The requested version is then cached in the state,
so the subsequent runs of the launcher will continue to execute the previously requested version.

If the `--force-version` parameter was not provided the launcher either tries to read its previously cached state
from disk, or assumes a fresh start.  On a fresh start, the launcher searches for the highest installed
version of `casper-node` and starts running it in validator mode.

After every run of the `casper-node` binary in validator mode, the launcher does the following based upon the exit code
returned by `casper-node`:
  * If 0 (success), searches for the immediate next installed version of `casper-node` and runs it in migrate-data mode
  * If 102 (downgrade), searches for the immediate previous installed version of `casper-node` and runs it in validator
    mode
  * If 103 (shutdown), runs the script at `/etc/casper/casper_shutdown_script` if present and exits with its exit code,
    otherwise exits with `0`.
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

## Number of Files Limit

When `casper-node` launches, it tries to set the `nofiles` for the process to `64000`.  With some systems, this will
hit the default hard limit of `4096`.

Filehandles are used for both files and network connections.  The network connections are unpredictable and running
out of file handles can stop critical file writes from occurring.  This limit may need to be increased from defaults.

With `casper-node-launcher` running we can see what the system allocated by finding our process id (PID) for casper-node
with `pgrep "casper-node$"`.

```shell
$ pgrep "casper-node$"
275928
```

This PID will change so you need to run the above command to get the current version with your system.  
It will not be `275928` each time. If you get no return, you do not have `casper-node-launcher` running properly.

To find the current `nofile` (number of open files) hard limit, we can run `prlimit` with this PID:

```shell
$ sudo prlimit -n -p 275928
RESOURCE DESCRIPTION              SOFT HARD UNITS
NOFILE   max number of open files 1024 4096 files
```

We can embed both commands together so it is only `sudo prlimit -n -p $(pgrep "casper-node$")`.

```shell
$ sudo prlimit -n -p $(pgrep "casper-node$")
RESOURCE DESCRIPTION              SOFT HARD UNITS
NOFILE   max number of open files 1024 4096 files
```

If you receive `prlimit: option requires an argument -- 'p'` with the above command then `pgrep "casper-node$"` is not
returning anything because `casper-node` is no longer running.

### Manual increase

This is how you set `nofile` for an active process.  It will make sure you don't have issues without having to 
restart the `casper-node-launcher` and your node's `casper-node` process.

We run `sudo prlimit --nofile=64000 --pid=$(pgrep "casper-node$")`.

After this when we look at `prlimit` it should show the change:

```shell
$ sudo prlimit -n -p $(pgrep "casper-node$")
RESOURCE DESCRIPTION               SOFT  HARD UNITS
NOFILE   max number of open files 64000 64000 files
```

This is only active while the `casper-node` process is active and therefore will not persist across server reboots, 
casper-node-launcher restarts, and protocol upgrades.  We need to do something else to make this permanent.

### limits.conf

Adding the `nofile` setting for `casper` user in `/etc/security/limits.conf` will persist this value.

Add:

`casper          hard    nofile          64000`

to the bottom of `/etc/security/limits.conf`.

After doing this you need to log out of any shells you have to enable this change. Restarting the node should
maintain the correct `nofile` setting.

### systemd unit modification (bad alternative)

When `casper-node-launcher` is installed, `/lib/systemd/system/casper-node-launcher.service` is created.  
Inside this file, a line is provided which will allow systemd to increase the `nofile` setting at launch.

`#LimitNOFILE=64000`

Editing this file with sudo will allow you to uncomment this line and save.  After saving you would need to run
`sudo systemctl daemon-reload` to reload your changes.  Then you would need to restart `casper-node-launcher`.

NOTE: The downside of using this method is that with upgrades to `casper-node-launcher`, the service file is replaced
and the update would not be persistent.  Editing `/etc/security/limits.conf` is a much preferred method.


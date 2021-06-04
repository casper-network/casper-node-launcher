#!/usr/bin/env python3

from pathlib import Path
import urllib3
import argparse
import enum

# protocol 1_0_0 should have accounts.toml
# All other protocols should have chainspec.toml, config.toml and NOT accounts.toml
# Protocols are typically shipped with config-example.toml to make config.toml
config_path = Path("/etc/casper")
bin_path = Path("/var/lib/casper/bin")
net_config_path = Path("/etc/casper/network_configs")
actions = ("check-for-upgrade", "stage-upgrades")


parser = argparse.ArgumentParser(description="Utility to install casper-node versions and troubleshoot.")
parser.add_argument('config_file', type=str, help=f"name of config file to use from {net_config_path}")
parser.add_argument('action', type=str, help=f"available actions: {actions}", choices=actions)


def get_config_values(file_name):
    expected_keys = ('SOURCE_URL', 'NETWORK_NAME')
    file_path = net_config_path / file_name
    config = {}
    for line in file_path.read_text().splitlines():
        key, value = line.strip().split('=')
        config[key] = value
    for key in expected_keys:
        if key not in config.keys():
            raise ValueError(f"Expected config value not found: {key} in {file_path}")
    return config


def get_protocol_versions(url, network):
    http = urllib3.PoolManager()
    full_url = f"{url}/{network}/protocol_versions"
    r = http.request('GET', full_url)
    if r.status != 200:
        raise IOError(f"Expected status 200 requesting {full_url}, received {r.status}")
    pv = r.data.decode('utf-8')
    return [data.strip() for data in pv.splitlines()]


class Status(enum.Enum):
    UNSTAGED = 1
    NO_CONFIG = 2
    BIN_ONLY = 3
    CONFIG_ONLY = 4
    STAGED = 5


STATUS_DISPLAY = {Status.UNSTAGED: "Protocol Unstaged",
                  Status.NO_CONFIG: "No config.toml for Protocol",
                  Status.BIN_ONLY: "Only bin is staged for Protocol, no config",
                  Status.CONFIG_ONLY: "Only config is staged for Protocol, no bin",
                  Status.STAGED: "Protocol Staged"}


def check_staged_version(version):
    config_version_path = config_path / version
    config_toml_file_path = config_version_path / "config.toml"
    bin_version_path = bin_path / version / "casper-node"
    if not config_version_path.exists():
        if not bin_version_path.exists():
            return Status.UNSTAGED
        return Status.BIN_ONLY
    else:
        if not bin_version_path.exists():
            return Status.CONFIG_ONLY
        if not config_toml_file_path.exists():
            return Status.NO_CONFIG
    return Status.STAGED


args = parser.parse_args()
config = get_config_values(args.config_file)
protocol_versions = get_protocol_versions(config["SOURCE_URL"], config["NETWORK_NAME"])
print(protocol_versions)
for pv in protocol_versions:
    status = check_staged_version(pv)
    print(f"{pv}: {STATUS_DISPLAY[status]}")

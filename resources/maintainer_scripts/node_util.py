#!/usr/bin/env python3
import ipaddress
import sys
from pathlib import Path
import urllib3
import argparse
import enum
import getpass
from ipaddress import ip_address
import tarfile
from collections import Counter
from shutil import chown
import os



# protocol 1_0_0 should have accounts.toml
# All other protocols should have chainspec.toml, config.toml and NOT accounts.toml
# Protocols are shipped with config-example.toml to make config.toml


class Status(enum.Enum):
    UNSTAGED = 1
    NO_CONFIG = 2
    BIN_ONLY = 3
    CONFIG_ONLY = 4
    STAGED = 5


class NodeUtil:
    """
    Using non `_` and non uppercase methods to expose for external commands.
    Description if command comes from the doc string of method.
    """
    CONFIG_PATH = Path("/etc/casper")
    BIN_PATH = Path("/var/lib/casper/bin")
    DB_PATH = Path("/var/lib/casper/casper-node")
    NET_CONFIG_PATH = CONFIG_PATH / "network_configs"
    SCRIPT_NAME = "node_util.py"

    def __init__(self):
        usage_docs = [f"{self.SCRIPT_NAME} <command> [args]", "Available commands:"]
        commands = []
        for function in [f for f in dir(self) if not f.startswith('_') and f[0].islower()]:
            usage_docs.append(f"  {function} - {getattr(self, function).__doc__.strip()}")
            commands.append(function)
        usage_docs.append(" ")

        self._http = urllib3.PoolManager()
        self._external_ip = None

        parser = argparse.ArgumentParser(
            description="Utility to help configure casper-node versions and troubleshoot.",
            usage="\n".join(usage_docs))
        parser.add_argument("command", help="Subcommand to run.", choices=commands)
        args = parser.parse_args(sys.argv[1:2])
        getattr(self, args.command)()

    def _get_config_values(self, config):
        """
        Parses config file to get values

        :param file_name: network config filename
        """
        source_url = "SOURCE_URL"
        network_name = "NETWORK_NAME"

        file_path = NodeUtil.NET_CONFIG_PATH / config
        expected_keys = (source_url, network_name)
        config = {}
        for line in file_path.read_text().splitlines():
            if line.strip():
                key, value = line.strip().split('=')
                config[key] = value
        for key in expected_keys:
            if key not in config.keys():
                raise ValueError(f"Expected config value not found: {key} in {file_path}")
        self.url = config[source_url]
        self.network = config[network_name]

    def _get_protocols(self):
        """ Downloads protocol versions for network """
        full_url = f"{self.url}/{self.network}/protocol_versions"
        r = self._http.request('GET', full_url)
        if r.status != 200:
            raise IOError(f"Expected status 200 requesting {full_url}, received {r.status}")
        pv = r.data.decode('utf-8')
        return [data.strip() for data in pv.splitlines()]

    @staticmethod
    def _verify_casper_user():
        if getpass.getuser() != "casper":
            print(f"Run with 'sudo -u casper'")
            exit(1)

    @staticmethod
    def _verify_root_user():
        if getpass.getuser() != "root":
            print("Run with 'sudo'")
            exit(1)

    @staticmethod
    def _status_text(status):
        status_display = {Status.UNSTAGED: "Protocol Unstaged",
                          Status.NO_CONFIG: "No config.toml for Protocol",
                          Status.BIN_ONLY: "Only bin is staged for Protocol, no config",
                          Status.CONFIG_ONLY: "Only config is staged for Protocol, no bin",
                          Status.STAGED: "Protocol Staged"}
        return status_display[status]

    @staticmethod
    def _check_staged_version(version):
        """
        Checks completeness of staged protocol version

        :param version: protocol version in underscore format such as 1_0_0
        :return: Status enum
        """
        config_version_path = NodeUtil.CONFIG_PATH / version
        config_toml_file_path = config_version_path / "config.toml"
        bin_version_path = NodeUtil.BIN_PATH / version / "casper-node"
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

    def _download_file(self, url, target_path):
        print(f"Downloading {url} to {target_path}")
        r = self._http.request('GET', url)
        if r.status != 200:
            raise IOError(f"Expected status 200 requesting {url}, received {r.status}")
        with open(target_path, 'wb') as f:
            f.write(r.data)

    @staticmethod
    def _extract_tar_gz(source_file_path, target_path):
        print(f"Extracting {source_file_path} to {target_path}")
        with tarfile.TarFile.open(source_file_path) as tf:
            for member in tf.getmembers():
                tf.extract(member, target_path)

    def _pull_protocol_version(self, protocol_version, rpm):
        self._verify_casper_user()

        if not NodeUtil.BIN_PATH.exists():
            print(f"Error: expected bin file location {NodeUtil.BIN_PATH} not found.")
            exit(1)

        if not NodeUtil.CONFIG_PATH.exists():
            print(f"Error: expected config file location {NodeUtil.CONFIG_PATH} not found.")
            exit(1)

        if rpm:
            bin_file = "bin_rpm.tar.gz"
        else:
            bin_file = "bin.tar.gz"
        config_file = "config.tar.gz"

        etc_full_path = NodeUtil.CONFIG_PATH / protocol_version
        bin_full_path = NodeUtil.BIN_PATH / protocol_version
        base_url = f"http://{self.url}/{self.network}/{protocol_version}"
        config_url = f"{base_url}/{config_file}"
        bin_url = f"{base_url}/{bin_file}"

        if etc_full_path.exists():
            print(f"Error: config version path {etc_full_path} already exists. Aborting.")
            exit(1)
        if bin_full_path.exists():
            print(f"Error: bin version path {bin_full_path} already exists. Aborting.")
            exit(1)

        config_archive_path = NodeUtil.CONFIG_PATH / config_file
        self._download_file(config_url, config_archive_path)
        self._extract_tar_gz(config_archive_path, etc_full_path)
        print(f"Deleting {config_archive_path}")
        config_archive_path.unlink()

        bin_archive_path = NodeUtil.BIN_PATH / bin_file
        self._download_file(bin_url, bin_archive_path)
        self._extract_tar_gz(bin_archive_path, bin_full_path)
        print(f"Deleting {bin_archive_path}")
        bin_archive_path.unlink()

    def _get_external_ip(self):
        if self._external_ip:
            return self._external_ip
        services = (("https://checkip.amazonaws.com", "amazonaws.com"),
                    ("https://ifconfig.me", "ifconfig.me"),
                    ("https://ident.me", "ident.me"))
        ips = []
        # Using our own PoolManager for shorter timeouts
        http = urllib3.PoolManager(timeout=10)
        print("Querying your external IP...")
        for url, service in services:
            r = http.request('GET', url, )
            if r.status != 200:
                ip = ""
            else:
                ip = r.data.decode('utf-8').strip()
            print(f" {service} says '{ip}' with Status: {r.status}")
            if ip:
                ips.append(ip)
        if ips:
            ip_addr = Counter(ips).most_common(1)[0][0]
            if self._is_valid_ip(ip_addr):
                self._external_ip = ip_addr
                return ip_addr
        return None

    @staticmethod
    def _is_valid_ip(ip):
        try:
            _ = ipaddress.ip_address(ip)
        except ValueError:
            return False
        else:
            return True

    def _config_from_example(self, protocol_version, ip=None):
        """ Create config.toml or config.toml.new (if previous exists) from config-example.toml"""
        self._verify_casper_user()

        config_path = NodeUtil.CONFIG_PATH / protocol_version
        config_toml_path = config_path / "config.toml"
        config_example = config_path / "config-example.toml"
        config_toml_new_path = config_path / "config.toml.new"

        if not config_example.exists():
            print(f"Error: {config_example} not found.")
            exit(1)

        if ip is None:
            ip = self._get_external_ip()
            print(f"Using detected ip: {ip}")
        else:
            print(f"Using provided ip: {ip}")

        if not self._is_valid_ip(ip):
            print(f"Error: Invalid IP: {ip}")
            exit(1)

        outfile = config_toml_path
        if config_toml_path.exists():
            outfile = config_toml_new_path
            print(f"Previous {config_toml_path} exists, creating as {outfile} from {config_example}.")
            print(f"Replace {config_toml_path} with {outfile} to use the automatically generated configuration.")

        outfile.write_text(config_example.read_text().replace("<IP ADDRESS>", ip))

    def stage_protocols(self):
        """Stage available protocols if needed (use 'sudo -u casper')"""
        parser = argparse.ArgumentParser(description=self.stage_protocols.__doc__,
                                         usage=f"{self.SCRIPT_NAME} stage_protocols [-h] config [--ip IP]")
        parser.add_argument("config", type=str, help=f"name of config file to use from {NodeUtil.NET_CONFIG_PATH}")
        parser.add_argument("--ip",
                            type=ip_address,
                            help=f"optional ip to use for config.toml instead of detected ip.",
                            required=False)
        parser.add_argument("--rpm",
                            action='store_true',
                            help="Optional flag to pull RPM compatible binaries.",
                            required=False)
        args = parser.parse_args(sys.argv[2:])
        self._get_config_values(args.config)

        self._verify_casper_user()

        exit_code = 0
        for pv in self._get_protocols():
            status = self._check_staged_version(pv)
            if status == Status.STAGED:
                print(f"{pv}: {self._status_text(status)}")
                continue
            elif status in (Status.BIN_ONLY, Status.CONFIG_ONLY):
                print(f"{pv}: {self._status_text(status)} - Not automatically recoverable.")
                exit_code = 1
                continue
            if status == Status.UNSTAGED:
                print(f"Pulling protocol for {pv}.")
                if not self._pull_protocol_version(pv, args.rpm):
                    exit_code = 1
            if status in (Status.UNSTAGED, Status.NO_CONFIG):
                print(f"Creating config for {pv}.")
                ip = str(args.ip) if args.ip else None
                if not self._config_from_example(pv, ip):
                    exit_code = 1
        exit(exit_code)

    def check_protocols(self):
        """ Checks if protocol are fully installed """
        parser = argparse.ArgumentParser(description=self.check_protocols.__doc__,
                                         usage=f"{self.SCRIPT_NAME} check_protocols [-h] config ")
        parser.add_argument("config", type=str, help=f"name of config file to use from {NodeUtil.NET_CONFIG_PATH}")
        args = parser.parse_args(sys.argv[2:])
        self._get_config_values(args.config)
        self._get_protocols()

        exit_code = 0
        for pv in self._get_protocols():
            status = self._check_staged_version(pv)
            if status != Status.STAGED:
                exit_code = 1
            print(f"{pv}: {self._status_text(status)}")
        exit(exit_code)

    def check_for_upgrade(self):
        """ Checks if protocol are fully installed """
        parser = argparse.ArgumentParser(description=self.check_for_upgrade.__doc__,
                                         usage=f"{self.SCRIPT_NAME} check_for_upgrade [-h] config ")
        parser.add_argument("config", type=str, help=f"name of config file to use from {NodeUtil.NET_CONFIG_PATH}")
        args = parser.parse_args(sys.argv[2:])
        self._get_config_values(args.config)
        last_protocol = self._get_protocols()[-1]
        status = self._check_staged_version(last_protocol)
        if status == Status.UNSTAGED:
            print(f"{last_protocol}: {self._status_text(status)}")
            exit(1)
        exit(0)

    @staticmethod
    def _is_casper_owned(path) -> bool:
        return path.owner() == 'casper' and path.group() == 'casper'

    @staticmethod
    def _walk_path(path, include_dir=True):
        for p in Path(path).iterdir():
            if p.is_dir():
                if include_dir:
                    yield p.resolve()
                yield from NodeUtil._walk_path(p)
                continue
            yield p.resolve()

    def check_permissions(self):
        """ Checking files are owned by casper. """
        # If a user runs commands under root, it can give files non casper ownership and cause problems.
        exit_code = 0
        for path in self._walk_path(NodeUtil.CONFIG_PATH):
            if not self._is_casper_owned(path):
                print(f"{path} is owned by {path.owner()}:{path.group()}")
                exit_code = 1
        for path in self._walk_path(NodeUtil.BIN_PATH):
            if not self._is_casper_owned(path):
                print(f"{path} is owned by {path.owner()}:{path.group()}")
                exit_code = 1
        exit(exit_code)

    def fix_permissions(self):
        """ Sets all files owner to casper (use 'sudo') """
        self._verify_root_user()

        exit_code = 0
        for path in self._walk_path(NodeUtil.CONFIG_PATH):
            if not self._is_casper_owned(path):
                print(f"Correcting ownership of {path}")
                chown(path, 'casper', 'casper')
                if not self._is_casper_owned(path):
                    print(f"Ownership set failed.")
                    exit_code = 1
        for path in self._walk_path(NodeUtil.BIN_PATH):
            if not self._is_casper_owned(path):
                print(f"Correcting ownership of {path}")
                chown(path, 'casper', 'casper')
                if not self._is_casper_owned(path):
                    print(f"Ownership set failed.")
                    exit_code = 1
        exit(exit_code)

    def rotate_logs(self):
        """ Rotate the logs for casper-node (use 'sudo') """
        self._verify_root_user()
        os.system("logrotate -f /etc/logrotate.d/casper-node")

    def delete_local_state(self):
        """ Delete local db and status files. (use 'sudo') """
        parser = argparse.ArgumentParser(description=self.delete_local_state.__doc__,
                                         usage=f"{self.SCRIPT_NAME} delete_local_state [-h] --verify-delete-all")
        parser.add_argument("--verify_delete_all",
                            action='store_true',
                            help="Required for verification that you want to delete everything",
                            required=False)
        args = parser.parse_args(sys.argv[2:])
        self._verify_root_user()

        if not args.verify_delete_all:
            print(f"Include '--verify_delete_all' flag to confirm. Exiting.")
            exit(1)

        for path in self.DB_PATH.glob('*'):
            path.unlink(missing_ok=True)
        cnl_state = self.CONFIG_PATH / "casper-node-launcher-state.toml"
        cnl_state.unlink(missing_ok=True)


NodeUtil()

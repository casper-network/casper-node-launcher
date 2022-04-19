#!/usr/bin/env python3
import ipaddress
import shutil
import sys
from pathlib import Path
from urllib import request
import argparse
import enum
import getpass
from ipaddress import ip_address
import tarfile
from collections import Counter
from shutil import chown
import os
import json
import time


# protocol 1_0_0 should have accounts.toml
# All other protocols should have chainspec.toml, config.toml and NOT accounts.toml
# Protocols are shipped with config-example.toml to make config.toml


class Status(enum.Enum):
    UNSTAGED = 1
    NO_CONFIG = 2
    BIN_ONLY = 3
    CONFIG_ONLY = 4
    STAGED = 5
    WRONG_NETWORK = 6


class NodeUtil:
    """
    Using non `_` and non uppercase methods to expose for external commands.
    Description of command comes from the doc string of method.
    """
    CONFIG_PATH = Path("/etc/casper")
    BIN_PATH = Path("/var/lib/casper/bin")
    DB_PATH = Path("/var/lib/casper/casper-node")
    NET_CONFIG_PATH = CONFIG_PATH / "network_configs"
    PLATFORM_PATH = CONFIG_PATH / "PLATFORM"
    SCRIPT_NAME = "node_util.py"
    NODE_IP = "127.0.0.1"

    def __init__(self):
        self._network_name = None
        self._url = None
        self._bin_mode = None

        usage_docs = [f"{self.SCRIPT_NAME} <command> [args]", "Available commands:"]
        commands = []
        for function in [f for f in dir(self) if not f.startswith('_') and f[0].islower()]:
            try:
                usage_docs.append(f"  {function} - {getattr(self, function).__doc__.strip()}")
            except AttributeError:
                raise Exception(f"Error creating usage docs, expecting {function} to be root function and have doc comment."
                                f" Lead with underscore if not.")
            commands.append(function)
        usage_docs.append(" ")

        self._external_ip = None

        parser = argparse.ArgumentParser(
            description="Utility to help configure casper-node versions and troubleshoot.",
            usage="\n".join(usage_docs))
        parser.add_argument("command", help="Subcommand to run.", choices=commands)
        args = parser.parse_args(sys.argv[1:2])
        getattr(self, args.command)()

    @staticmethod
    def _rpc_call(method: str, server: str, params: list, port: int = 7777, timeout: int = 5):
        url = f"http://{server}:{port}/rpc"
        req = request.Request(url, method="POST")
        req.add_header('content-type', "application/json")
        req.add_header('cache-control', "no-cache")
        payload = json.dumps({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).encode('utf-8')
        r = request.urlopen(req, payload, timeout=timeout)
        json_data = json.loads(r.read())
        return json_data["result"]

    @staticmethod
    def _rpc_get_block(server: str, block_height=None, port: int = 7777, timeout: int = 5):
        """
        Get block based on block_hash, block_height, or last block if block_identifier is missing
        """
        params = []
        if block_height:
            params = [{"Height": int(block_height)}]
        return NodeUtil._rpc_call("chain_get_block", server, params, port)

    @staticmethod
    def _get_platform():
        """ Support old default debian and then newer platforms with PLATFORM files """
        if NodeUtil.PLATFORM_PATH.exists():
            return NodeUtil.PLATFORM_PATH.read_text().strip()
        else:
            return "deb"

    def _load_config_values(self, config):
        """
        Parses config file to get values

        :param file_name: network config filename
        """
        source_url = "SOURCE_URL"
        network_name = "NETWORK_NAME"
        bin_mode = "BIN_MODE"

        file_path = NodeUtil.NET_CONFIG_PATH / config
        expected_keys = (source_url, network_name)
        config = {}
        for line in file_path.read_text().splitlines():
            if line.strip():
                key, value = line.strip().split('=')
                config[key] = value
        for key in expected_keys:
            if key not in config.keys():
                print(f"Expected config value not found: {key} in {file_path}")
                exit(1)
        self._url = config[source_url]
        self._network_name = config[network_name]
        self._bin_mode = config.get(bin_mode, "mainnet")

    def _get_protocols(self):
        """ Downloads protocol versions for network """
        full_url = f"{self._network_url}/protocol_versions"
        r = request.urlopen(full_url)
        if r.status != 200:
            raise IOError(f"Expected status 200 requesting {full_url}, received {r.status}")
        pv = r.read().decode('utf-8')
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
                          Status.WRONG_NETWORK: "chainspec.toml is for wrong network",
                          Status.STAGED: "Protocol Staged"}
        return status_display[status]

    def _check_staged_version(self, version):
        """
        Checks completeness of staged protocol version

        :param version: protocol version in underscore format such as 1_0_0
        :return: Status enum
        """
        if not self._network_name:
            print("Config not parsed prior to call of _check_staged_version and self._network_name is not populated.")
            exit(1)
        config_version_path = NodeUtil.CONFIG_PATH / version
        chainspec_toml_file_path = config_version_path / "chainspec.toml"
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
            if NodeUtil._chainspec_name(chainspec_toml_file_path) != self._network_name:
                return Status.WRONG_NETWORK
        return Status.STAGED

    @staticmethod
    def _download_file(url, target_path):
        print(f"Downloading {url} to {target_path}")
        r = request.urlopen(url)
        if r.status != 200:
            raise IOError(f"Expected status 200 requesting {url}, received {r.status}")
        with open(target_path, 'wb') as f:
            f.write(r.read())

    @staticmethod
    def _extract_tar_gz(source_file_path, target_path):
        print(f"Extracting {source_file_path} to {target_path}")
        with tarfile.TarFile.open(source_file_path) as tf:
            for member in tf.getmembers():
                tf.extract(member, target_path)

    @property
    def _network_url(self):
        return f"http://{self._url}/{self._network_name}"

    def _pull_protocol_version(self, protocol_version, platform="deb"):
        self._verify_casper_user()

        if not NodeUtil.BIN_PATH.exists():
            print(f"Error: expected bin file location {NodeUtil.BIN_PATH} not found.")
            exit(1)

        if not NodeUtil.CONFIG_PATH.exists():
            print(f"Error: expected config file location {NodeUtil.CONFIG_PATH} not found.")
            exit(1)

        # Expectation is one config.tar.gz but multiple bin*.tar.gz
        # bin.tar.gz is mainnet bin and debian
        # bin_new.tar.gz is post 1.4.0 launch and debian
        # bin_rpm.tar.gz is mainnet bin and RHEL (_arch will be used for others in the future)
        # bin_rpm_new.tar.gz is post 1.4.0 launch and RHEL

        bin_file = "bin"
        if platform != "deb":
            # Handle alternative builds
            bin_file += f"_{platform}"
        if self._bin_mode != "mainnet":
            # Handle non mainnet for post 1.4.0 launched networks
            bin_file += "_new"
        bin_file += ".tar.gz"
        config_file = "config.tar.gz"
        print(f"Using bin mode file of {bin_file}")

        etc_full_path = NodeUtil.CONFIG_PATH / protocol_version
        bin_full_path = NodeUtil.BIN_PATH / protocol_version
        base_url = f"{self._network_url}/{protocol_version}"
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
        return True

    def _get_external_ip(self):
        """ Query multiple sources to get external IP of node """
        if self._external_ip:
            return self._external_ip
        services = (("https://checkip.amazonaws.com", "amazonaws.com"),
                    ("https://ifconfig.me", "ifconfig.me"),
                    ("https://ident.me", "ident.me"))
        ips = []
        # Using our own PoolManager for shorter timeouts
        print("Querying your external IP...")
        for url, service in services:
            r = request.urlopen(url)
            if r.status != 200:
                ip = ""
            else:
                ip = r.read().decode('utf-8').strip()
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
        """ Check validity of ip address """
        try:
            _ = ipaddress.ip_address(ip)
        except ValueError:
            return False
        else:
            return True

    @staticmethod
    def _toml_header(line_data):
        data = line_data.strip()
        length = len(data)
        if data[0] == '[' and data[length - 1] == ']':
            return data[1:length - 1]
        return None

    @staticmethod
    def _toml_name_value(line_data):
        data = line_data.strip().split(' = ')
        if len(data) != 2:
            raise ValueError(f"Expected `name = value` with _toml_name_value for {line_data}")
        return data

    @staticmethod
    def _is_toml_comment_or_empty(line_data):
        data = line_data.strip()
        if len(data) == 0:
            return True
        if data[0] == '#':
            return True
        return False

    @staticmethod
    def _replace_config_values(config_data, replace_file):
        """ Replace values in config_data with values for fields in replace_file """
        replace_file_path = Path(replace_file)
        if not replace_file_path.exists():
            raise ValueError(f"Cannot replace values in config, {replace_file} does not exist.")
        replace_data = replace_file_path.read_text().splitlines()
        replacements = []
        last_header = None
        for line in replace_data:
            if NodeUtil._is_toml_comment_or_empty(line):
                continue
            header = NodeUtil._toml_header(line)
            if header is not None:
                last_header = header
                continue
            name, value = NodeUtil._toml_name_value(line)
            replacements.append((last_header, name, value))
        new_output = []
        last_header = None
        for line in config_data.splitlines():
            if NodeUtil._is_toml_comment_or_empty(line):
                new_output.append(line)
                continue
            header = NodeUtil._toml_header(line)
            if header is not None:
                last_header = header
                new_output.append(line)
                continue
            name, value = NodeUtil._toml_name_value(line)
            replacement_value = [r_value for r_header, r_name, r_value in replacements
                                 if last_header == r_header and name == r_name]
            if replacement_value:
                new_value = replacement_value[0]
                print(f"Replacing {last_header}:{name} = {value} with {new_value}")
                new_output.append(f"{name} = {new_value}")
            else:
                new_output.append(line)
        return "\n".join(new_output)

    def _config_from_example(self, protocol_version, ip=None, replace_toml=None):
        """
        Internal Method to allow use in larger actions or direct call from config_from_example.
        Create config.toml or config.toml.new (if previous exists) from config-example.toml
        """
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

        config_text = config_example.read_text()
        if replace_toml is not None:
            config_text = NodeUtil._replace_config_values(config_text, replace_toml)

        outfile.write_text(config_text.replace("<IP ADDRESS>", ip))
        
        return True

    def config_from_example(self):
        """ Create config.toml from config-example.toml. (use 'sudo -u casper') """
        parser = argparse.ArgumentParser(description=self.config_from_example.__doc__,
                                         usage=(f"{self.SCRIPT_NAME} config_from_example [-h] "
                                                "protocol_version [--replace replace_file.toml] [--ip IP]"))
        parser.add_argument("protocol_version", type=str, help=f"protocol version to create under")
        parser.add_argument("--ip",
                            type=ip_address,
                            help=f"optional ip to use for config.toml instead of detected ip.",
                            required=False)
        parser.add_argument("--replace",
                            type=str,
                            help=("optional toml file that holds replacements to make to config.toml "
                                  "from config-example.toml"),
                            required=False)
        args = parser.parse_args(sys.argv[2:])
        ip = str(args.ip) if args.ip else None
        self._config_from_example(args.protocol_version, ip, args.replace)

    def stage_protocols(self):
        """Stage available protocols if needed (use 'sudo -u casper')"""
        parser = argparse.ArgumentParser(description=self.stage_protocols.__doc__,
                                         usage=(f"{self.SCRIPT_NAME} stage_protocols [-h] config "
                                                "[--ip IP] [--replace toml_file]"))
        parser.add_argument("config", type=str, help=f"name of config file to use from {NodeUtil.NET_CONFIG_PATH}")
        parser.add_argument("--ip",
                            type=ip_address,
                            help=f"optional ip to use for config.toml instead of detected ip.",
                            required=False)
        parser.add_argument("--replace",
                            type=str,
                            help=("optional toml file that holds replacements to make to config.toml "
                                  "from config-example.toml"),
                            required=False)
        args = parser.parse_args(sys.argv[2:])
        self._load_config_values(args.config)

        self._verify_casper_user()
        platform = self._get_platform()
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
                if not self._pull_protocol_version(pv, platform):
                    exit_code = 1
            if status in (Status.UNSTAGED, Status.NO_CONFIG):
                print(f"Creating config for {pv}.")
                ip = str(args.ip) if args.ip else None
                if not self._config_from_example(pv, ip, args.replace):
                    exit_code = 1
        exit(exit_code)

    def check_protocols(self):
        """ Checks if protocol are fully installed """
        parser = argparse.ArgumentParser(description=self.check_protocols.__doc__,
                                         usage=f"{self.SCRIPT_NAME} check_protocols [-h] config ")
        parser.add_argument("config", type=str, help=f"name of config file to use from {NodeUtil.NET_CONFIG_PATH}")
        args = parser.parse_args(sys.argv[2:])
        self._load_config_values(args.config)

        exit_code = 0
        for pv in self._get_protocols():
            status = self._check_staged_version(pv)
            if status != Status.STAGED:
                exit_code = 1
            print(f"{pv}: {self._status_text(status)}")
        exit(exit_code)

    def check_for_upgrade(self):
        """ Checks if last protocol is staged """
        parser = argparse.ArgumentParser(description=self.check_for_upgrade.__doc__,
                                         usage=f"{self.SCRIPT_NAME} check_for_upgrade [-h] config ")
        parser.add_argument("config", type=str, help=f"name of config file to use from {NodeUtil.NET_CONFIG_PATH}")
        args = parser.parse_args(sys.argv[2:])
        self._load_config_values(args.config)
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
    def _walk_file_locations():
        for path in NodeUtil.BIN_PATH, NodeUtil.CONFIG_PATH, NodeUtil.DB_PATH:
            for _path in NodeUtil._walk_path(path):
                yield _path

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
        for path in self._walk_file_locations():
            if not self._is_casper_owned(path):
                print(f"{path} is owned by {path.owner()}:{path.group()}")
                exit_code = 1
        if exit_code == 0:
            print("Permissions are correct.")
        exit(exit_code)

    def fix_permissions(self):
        """ Sets all files owner to casper (use 'sudo') """
        self._verify_root_user()

        exit_code = 0
        for path in self._walk_file_locations():
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

    def restart(self):
        """ Restart casper-node-launcher (use 'sudo) """
        # Using stop, pause, start to get full reload not done with systemctl restart
        self.stop()
        time.sleep(1)
        self.start()

    def stop(self):
        """ Stop casper-node-launcher (use 'sudo') """
        self._verify_root_user()
        os.system("systemctl stop casper-node-launcher")

    def start(self):
        """ Start casper-node-launcher (use 'sudo') """
        self._verify_root_user()
        os.system("systemctl start casper-node-launcher")

    @staticmethod
    def systemd_status():
        """ Status of casper-node-launcher """
        # using op.popen to stop hanging return to terminate
        result = os.popen("systemctl status casper-node-launcher")
        print(result.read())

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

        # missing_ok=True arg to unlink only 3.8+, using try/catch.
        for path in self.DB_PATH.glob('*'):
            try:
                if path.is_dir():
                    shutil.rmtree(path)
                else:
                    path.unlink()
            except FileNotFoundError:
                pass
        cnl_state = self.CONFIG_PATH / "casper-node-launcher-state.toml"
        try:
            cnl_state.unlink()
        except FileNotFoundError:
            pass

    def force_run_version(self):
        """ Force casper-node-launcher to start at a certain protocol version """
        parser = argparse.ArgumentParser(description=self.force_run_version.__doc__,
                                         usage=f"{self.SCRIPT_NAME} force_run_version [-h] protocol_version")
        parser.add_argument("protocol_version", help="Protocol version for casper-node-launcher to run.")
        args = parser.parse_args(sys.argv[2:])
        version = args.protocol_version
        config_path = self.CONFIG_PATH / version
        bin_path = self.BIN_PATH / version
        if not config_path.exists():
            print(f"/etc/casper/{version} not found.  Aborting.")
            exit(1)
        if not bin_path.exists():
            print(f"/var/lib/casper/bin/{version} not found.  Aborting.")
            exit(1)
        # Need to be root to restart below
        self._verify_root_user()
        state_path = self.CONFIG_PATH / "casper-node-launcher-state.toml"
        lines = ["mode = 'RunNodeAsValidator'",
                 f"version = '{version.replace('_','.')}'",
                 f"binary_path = '/var/lib/casper/bin/{version}/casper-node'",
                 f"config_path = '/etc/casper/{version}/config.toml'"]
        state_path.write_text("\n".join(lines))
        # Make file casper:casper owned
        import pwd
        user = pwd.getpwnam('casper')
        os.chown(state_path, user.pw_uid, user.pw_gid)
        self.restart()

    @staticmethod
    def _ip_address_type(ip_address: str):
        """ Validation method for argparse """
        try:
            ip = ipaddress.ip_address(ip_address)
        except ValueError:
            print(f"Error: Invalid IP: {ip_address}")
        else:
            return str(ip)

    @staticmethod
    def _get_status(ip=None, port=8888):
        """ Get status data from node """
        if ip is None:
            ip = NodeUtil.NODE_IP
        full_url = f"http://{ip}:{port}/status"
        r = request.urlopen(full_url, timeout=5)
        return json.loads(r.read().decode('utf-8'))

    @staticmethod
    def _chainspec_name(chainspec_path) -> str:
        # Hack to not require toml package install
        for line in chainspec_path.read_text().splitlines():
            NAME_DATA = "name = '"
            if line[:len(NAME_DATA)] == NAME_DATA:
                return line.split(NAME_DATA)[1].split("'")[0]

    @staticmethod
    def _format_status(status, external_block_data=None):
        try:
            if status is None:
                status = {}
            error = status.get("error")
            if error:
                return f"status error: {error}"
            block_info = status.get("last_added_block_info")
            output = []
            if block_info is not None:
                cur_block = block_info.get('height')
                output.append(f"Last Block: {cur_block} (Era: {block_info.get('era_id')})")
                if external_block_data is not None:
                    if len(external_block_data) > 1:
                        output.append(f" Tip Block: {external_block_data[0]} (Era: {external_block_data[1]})")
                        output.append(f"    Behind: {external_block_data[0] - cur_block}")
                        output.append("")
            output.extend([
                f"Peer Count: {len(status.get('peers', []))}",
                f"Uptime: {status.get('uptime', '')}",
                f"Build: {status.get('build_version')}",
                f"Key: {status.get('our_public_signing_key')}",
                f"Next Upgrade: {status.get('next_upgrade')}",
                ""
            ])
            return "\n".join(output)
        except Exception:
            return "Cannot parse status return."

    @staticmethod
    def _ip_status_height(ip):
        try:
            status = NodeUtil._get_status(ip=ip)
            block_info = status.get("last_added_block_info")
            if block_info is None:
                return None
            return block_info.get('height'), block_info.get('era_id')
        except Exception:
            return None

    def node_status(self):
        """ Get full status of node """

        parser = argparse.ArgumentParser(description=self.watch.__doc__,
                                         usage=f"{self.SCRIPT_NAME} node_status [-h] [--ip]")
        parser.add_argument("--ip", help="ip address of a node at the tip",
                            type=self._ip_address_type, required=False)
        args = parser.parse_args(sys.argv[2:])

        try:
            status = self._get_status()
        except Exception as e:
            status = {"error": e}
        external_block_data = None
        if args.ip:
            external_block_data = self._ip_status_height(str(args.ip))
        print(self._format_status(status, external_block_data))

    def watch(self):
        """ watch full_node_status """
        DEFAULT = 5
        MINIMUM = 5

        parser = argparse.ArgumentParser(description=self.watch.__doc__,
                                         usage=f"{self.SCRIPT_NAME} watch [-h] [-r] [--ip]")
        parser.add_argument("-r", "--refresh", help="Refresh time in secs", type=int, default=DEFAULT, required=False)
        parser.add_argument("--ip", help="ip address of a node at the tip",
                            type=self._ip_address_type, required=False)
        args = parser.parse_args(sys.argv[2:])
        ip_arg = ""
        if args.ip:
            ip_arg = f"--ip {str(args.ip)}"
        refresh = MINIMUM if args.refresh < MINIMUM else args.refresh
        os.system(f"watch -n {refresh} '{sys.argv[0]} node_status {ip_arg}; {sys.argv[0]} rpc_active; {sys.argv[0]} systemd_status'")

    def rpc_active(self):
        """ Is local RPC active? """
        try:
            block = self._rpc_get_block("127.0.0.1", timeout=1)
            print("RPC: Ready\n")
            exit(0)
        except Exception:
            print("RPC: Not Ready\n")
            exit(1)

    def get_trusted_hash(self):
        """ Retrieve trusted hash from given node ip while verifying network """
        parser = argparse.ArgumentParser(description=self.get_trusted_hash.__doc__,
                                         usage=f"{self.SCRIPT_NAME} get_trusted_hash ip [--protocol] [--block]")
        parser.add_argument("ip",
                            help="Trusted Node IP address ipv4 format",
                            type=self._ip_address_type)
        parser.add_argument("--protocol",
                            help="Protocol version for chainspec to verify same network",
                            required=False,
                            default="1_0_0")
        parser.add_argument("--block",
                            help="Block number to use (latest if omitted)",
                            required=False)

        args = parser.parse_args(sys.argv[2:])

        status = None
        try:
            status = self._get_status(args.ip)
        except Exception as e:
            print(f"Error retrieving status from {args.ip}: {e}")
            exit(1)

        remote_network_name = status["chainspec_name"]
        chainspec_path = Path("/etc/casper") / args.protocol / "chainspec.toml"

        if not chainspec_path.exists():
            print(f"Cannot find {chainspec_path}, specify valid protocol folder to verify network name.")
            exit(1)

        chainspec_name = self._chainspec_name(chainspec_path)
        if chainspec_name != remote_network_name:
            print(f"Node network name: '{remote_network_name}' does not match {chainspec_path}: '{chainspec_name}'")
            exit(1)

        last_added_block_info = status["last_added_block_info"]
        if last_added_block_info is None:
            print(f"No last_added_block_info in {args.ip} status. Node is not in sync and will not be used.")
            exit(1)
        # If no block, getting latest so store now
        block_hash = last_added_block_info["hash"]
        if args.block:
            try:
                block = self._rpc_get_block(server=args.ip, block_height=args.block)
                block_hash = block["block"]["hash"]
            except Exception as e:
                if "timed out" in str(e):
                    print(f"RPC call timed out, either {args.ip} has RPC port blocked or is not in sync.")
                else:
                    print(f"Error calling RPC for {args.ip}.")
                exit(1)
        print(f"{block_hash}")


if __name__ == '__main__':
    NodeUtil()

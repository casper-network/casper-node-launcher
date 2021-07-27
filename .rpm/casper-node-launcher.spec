BuildRequires: systemd-rpm-macros
Requires: curl
%define __spec_install_post %{nil}
%define __os_install_post %{_dbpath}/brp-compress
%define debug_package %{nil}

Name: casper-node-launcher
Summary: A binary which runs and upgrades the casper-node of the Casper network
Version: @@VERSION@@
Release: @@RELEASE@@%{?dist}
License: CasperLabs Open Source License (COSL)
Group: Applications/System
Source0: %{name}-%{version}.tar.gz
URL: https://casperlabs.io

BuildRoot: %{_tmppath}/%{name}-%{version}-%{release}-root

%description
%{summary}

%prep
%setup -q

%pre
# Default Variables
# ---
DEFAULT_USERNAME="casper"
DEFAULT_CONFIG_DIRECTORY="/etc/${DEFAULT_USERNAME}"
DEFAULT_DATA_DIRECTORY="/var/lib/${DEFAULT_USERNAME}/bin"
DEFAULT_LOG_DIRECOTRY="/var/log/${DEFAULT_USERNAME}"

# Creation of Files/Directories
# ---
# Assure DEFAULT_DATA_DIRECTORY is available for state data
if [ -d ${DEFAULT_DATA_DIRECTORY} ] ; then
    echo "Directory ${DEFAULT_DATA_DIRECTORY} already exists."
else
    mkdir -p ${DEFAULT_DATA_DIRECTORY}
fi

# Assure DEFAULT_CONFIG_DIRECTORY is available for config data
if [ -d ${DEFAULT_CONFIG_DIRECTORY} ] ; then
    echo "Directory ${DEFAULT_CONFIG_DIRECTORY} already exists."
else
    mkdir -p ${DEFAULT_CONFIG_DIRECTORY}
fi

# Assure DEFAULT_LOG_DIRECOTRY is available for logging
if [ -d ${DEFAULT_LOG_DIRECOTRY} ] ; then
    echo "Directory ${DEFAULT_LOG_DIRECOTRY} already exists."
else
    mkdir -p ${DEFAULT_LOG_DIRECOTRY}
fi
exit 0

%post
# Default Variables
# ---
DEFAULT_USERNAME="casper"
DEFAULT_CONFIG_DIRECTORY="/etc/${DEFAULT_USERNAME}"
DEFAULT_DATA_DIRECTORY="/var/lib/${DEFAULT_USERNAME}"
DEFAULT_LOG_DIRECOTRY="/var/log/${DEFAULT_USERNAME}"

# User Creation
# ---
# Assure DEFAULT_USERNAME user exists
getent group casper >/dev/null || groupadd -r casper
getent passwd casper >/dev/null || \
    useradd -r -g casper -s /sbin/nologin \
    -c "User for running casper-node-launcher" casper

# Take ownership of directories and files installed
chown -R ${DEFAULT_USERNAME}:${DEFAULT_USERNAME} ${DEFAULT_DATA_DIRECTORY}
chown -R ${DEFAULT_USERNAME}:${DEFAULT_USERNAME} ${DEFAULT_CONFIG_DIRECTORY}
chown -R ${DEFAULT_USERNAME}:${DEFAULT_USERNAME} ${DEFAULT_LOG_DIRECOTRY}

# Update systemd for unit file
systemctl daemon-reload

exit 0


%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a * %{buildroot}

%clean
rm -rf %{buildroot}

%files
%defattr(-,casper,casper,-)
%{_bindir}/*
/var/lib/casper/bin/README.md
/etc/logrotate.d/casper-node
/etc/casper/PLATFORM
/etc/casper/pull_casper_node_version.sh
/etc/casper/network_configs/casper.conf
/etc/casper/network_configs/casper-test.conf
/etc/casper/delete_local_db.sh
/etc/casper/config_from_example.sh
/etc/casper/README.md
/etc/casper/validator_keys/README.md
/etc/systemd/system/casper-node-launcher.service

#!/usr/bin/env bash
set -e

# This script will generate a CONFIG file appropriate to installation machine.

echo "This script is deprecated and will be removed. - Use node_util.py config_from_example."

USERNAME=casper

if [ "$(whoami)" != "$USERNAME" ]; then
  echo
  echo "Script must be run as user: $USERNAME"
  echo "Do this with 'sudo -u $USERNAME $0 <protocol_version> [optional external IP]'"
  echo
  exit 1
fi

if [ -z "$1" ]; then
  echo
  echo "Error: version argument missing."
  echo "config-example.toml should exist in a given /etc/casper/[version] folder."
  echo ""
  echo "Ex: for version 1.0.1 of casper-node, /etc/casper/1_0_1/config-example.toml should exist."
  echo "    Should be called with '${0} 1_0_1'"
  echo
  exit 1
fi

CONFIG_PATH="/etc/casper/$1"
CONFIG="$CONFIG_PATH/config.toml"
CONFIG_EXAMPLE="$CONFIG_PATH/config-example.toml"
CONFIG_NEW="$CONFIG_PATH/config.toml.new"

if [ ! -f "$CONFIG_EXAMPLE" ]; then
  echo
  echo "Error: $CONFIG_EXAMPLE not found."
  echo
  exit 2
fi

function valid_ip()
{
  local  ip=$1
  local  stat=1

  if [[ $ip =~ ^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$ ]]; then
    OIFS=$IFS
    IFS='.'
    ip=($ip)
    IFS=$OIFS
    [[ ${ip[0]} -le 255 && ${ip[1]} -le 255 \
        && ${ip[2]} -le 255 && ${ip[3]} -le 255 ]]
    stat=$?
  fi
  return $stat
}

if [ -z "$2" ]; then
  # IP to be detected
  function curl_ext_ip()
  {
    result=$(curl -s --max-time 10 --connect-timeout 10 "$1") || result='dead pipe'
  }

  URLS=("https://checkip.amazonaws.com" "https://ifconfig.me" "https://ident.me")
  NAMES=("amazonaws.com" "ifconfig.me" "ident.me")
  RESULTS=()
  array_len=${#URLS[@]}

  echo "Trying to get external IP from couple of services ..."

  for (( i=0; i<$array_len; i++ )); do
    curl_ext_ip "${URLS[$i]}"
    if [[ $result != "dead pipe" ]]; then
      RESULTS+=($result)
    fi
    echo "${NAMES[$i]} report: $result"
  done

  IPv4_STRING='(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)'
  EXTERNAL_IP=$(echo "${RESULTS[@]}" | awk '{for(i=1;i<=NF;i++) print $i}' | awk '!x[$0]++' | grep -E -o "$IPv4_STRING" | head -n 1)

  if valid_ip $EXTERNAL_IP; then
   echo "Using External IP: $EXTERNAL_IP"
  else
   echo
   echo "WARNING: Can't get external VPS IP automatically."
   echo "Run script again with '$0 $1 <external ip address>'"
   echo
   exit 3
  fi

else
  # IP passed into script
  EXTERNAL_IP=$2
  if valid_ip $EXTERNAL_IP; then
    echo "Using provided IP: $EXTERNAL_IP"
  else
    echo
    echo "Error: Provided IP: $EXTERNAL_IP is invalid. Expected IPv4 address."
    echo
    exit 4
  fi

fi

OUTFILE=$CONFIG

if [[ -f $OUTFILE ]]; then
 OUTFILE=$CONFIG_NEW
 if [[ -f $OUTFILE ]]; then
   rm $OUTFILE
 fi
 echo "Previous $CONFIG exists, creating as $OUTFILE from $CONFIG_EXAMPLE."
 echo "Replace $CONFIG with $OUTFILE to use the automatically generated configuration."
else
 echo "Creating $OUTFILE from $CONFIG_EXAMPLE."
fi

sed "s/<IP ADDRESS>/${EXTERNAL_IP}/" $CONFIG_EXAMPLE > $OUTFILE

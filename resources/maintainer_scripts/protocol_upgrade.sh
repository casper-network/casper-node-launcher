#!/usr/bin/env bash

set -e

# This script will pull the protocol_versions file stored in the root of the staged file source
# and determine if an upgrade is needing to be staged.

USERNAME=casper
ARGUMENT_EXAMPLES="<config filename in network_configs dir> [stage | check]"

if [ -z "$1" ]; then
  echo
  echo "Error: arguments missing"
  echo "Expected $0 $ARGUMENT_EXAMPLES"
  echo "Example: $0 casper.conf check"
  echo
  exit 1
fi

if [ -z "$2" ]; then
  echo
  echo "Error: arguments missing"
  echo "Expected $0 $ARGUMENT_EXAMPLES"
  echo "Example: $0 casper.conf check"
  echo
  exit 1
fi


MODE=$2
if [[ "$MODE" != "stage" && "$MODE" != "check" ]]; then
  echo
  echo "Error: Illegal operation. Expected $0 $ARGUMENT_EXAMPLES"
  echo
  exit 1
fi
CONFIG="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )/network_configs/$1"

if [ ! -f "$CONFIG" ]; then
  echo
  echo "Config file given: $CONFIG does not exist."
  echo
  exit 1
fi

# This should set SOURCE_URL and NETWORK_NAME vars
source "$CONFIG"

if [ "$SOURCE_URL" == "" ]; then
  echo
  echo "Error: source_url not set and expected from '$CONFIG'."
  echo
  exit 1
fi

if [ "$NETWORK_NAME" == "" ]; then
  echo
  echo "Error: network_name not set and expected from '$CONFIG'."
  echo
  exit 1
fi

ETC_PATH="/etc/casper"
BIN_PATH="/var/lib/casper/bin"

if [ ! -d "$ETC_PATH" ]; then
  echo
  echo "Error: expected config file location $ETC_PATH not found."
  echo
  exit 1
fi

if [ ! -d "$BIN_PATH" ]; then
  echo
  echo "Error: expected bin file location $BIN_PATH not found."
  echo
  exit 1
fi

GOOD="good"
CONFIG_MISSING="config is missing"
NOT_STAGED="upgrade not staged"

protocol_version_staged() {
  local result=$GOOD
  if [ -d "/etc/casper/$1" ]; then
    if [ ! -f "/etc/casper/$1/config.toml" ]; then
      local result=$CONFIG_MISSING
    fi
  else
    local result=$NOT_STAGED
  fi
  echo "$result"
}

process_protocol_version() {
  staged=$(protocol_version_staged $1)
  if [ $staged == $GOOD ]; then
    echo "$1 staged"
  else
    if [ $staged == $CONFIG_MISSING ]; then
      echo "$1 staged, but config.toml missing."

  fi

  echo $1
}

# If curl URL is wrong we will exit 22 with curl 404.
while read protocol_version; do \
  process_protocol_version "$protocol_version"
done < <(curl -sSf "$SOURCE_URL/$NETWORK_NAME_/protocol_versions")

#ETC_FULL_PATH="$ETC_PATH/$SEMVER"
#BIN_FULL_PATH="$BIN_PATH/$SEMVER"
#
#BASE_URL="http://$SOURCE_URL/$NETWORK_NAME/$SEMVER"
#CONFIG_ARCHIVE="config.tar.gz"
#CONFIG_URL="$BASE_URL/$CONFIG_ARCHIVE"
#BIN_ARCHIVE="bin.tar.gz"
#BIN_URL="$BASE_URL/$BIN_ARCHIVE"
#
#cd $ETC_PATH
#
#echo "Verifying semver Path"
#curl -I 2>/dev/null "$CONFIG_URL" | head -1 | grep 404 >/dev/null
#if [ $? == 0 ]; then
#  echo
#  echo "$CONFIG_URL not found.  Please verify provided arguments"
#  echo
#  exit 10
#fi
#curl -I 2>/dev/null "$BIN_URL" | head -1 | grep 404 >/dev/null
#if [ $? == 0 ]; then
#  echo
#  echo "$BIN_URL not found.  Please verify provided arguments"
#  echo
#  exit 11
#fi
#
#if [ -d "$ETC_FULL_PATH" ]; then
#  echo
#  echo "Error: config version path $ETC_FULL_PATH already exists. Aborting."
#  echo
#  exit 12
#fi
#
#if [ -d "$BIN_FULL_PATH" ]; then
#  echo
#  echo "Error: bin version path $BIN_FULL_PATH already exists. Aborting."
#  echo
#  exit 13
#fi
#
#echo "Downloading $CONFIG_ARCHIVE from $CONFIG_URL"
#if curl -JLO "$CONFIG_URL"; then
#  echo "Complete"
#else
#  echo "Error: unable to pull $CONFIG_ARCHIVE from $CONFIG_URL."
#  echo "File probably doesn't exist.  Please verify provided arguments"
#  exit 14
#fi
#CONFIG_ARCHIVE_PATH="$ETC_PATH/$CONFIG_ARCHIVE"
#
#echo "Downloading $BIN_ARCHIVE from $BIN_URL"
#if curl -JLO "$BIN_URL"; then
#  echo "Complete"
#else
#  echo "Error: unable to pull $BIN_ARCHIVE from $BIN_URL"
#  echo "File probably doesn't exist.  Please verify provided arguments"
#  exit 15
#fi
#BIN_ARCHIVE_PATH="$ETC_PATH/$BIN_ARCHIVE"
#
#echo "Extracting $BIN_ARCHIVE to $BIN_FULL_PATH"
#mkdir -p "$BIN_FULL_PATH"
#cd "$BIN_FULL_PATH"
#tar -xzvf "$BIN_ARCHIVE_PATH" .
#
#echo "Extracting $CONFIG_ARCHIVE to $ETC_FULL_PATH"
#mkdir -p "$ETC_FULL_PATH"
#cd "$ETC_FULL_PATH"
#tar -xzvf "$CONFIG_ARCHIVE_PATH"
#
#echo "Removing $BIN_ARCHIVE_PATH"
#rm "$BIN_ARCHIVE_PATH"
#echo "Removing $CONFIG_ARCHIVE_PATH"
#rm "$CONFIG_ARCHIVE_PATH"
#
#echo "Process Complete."
#

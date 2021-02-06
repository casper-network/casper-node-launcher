#!/usr/bin/env bash

set -e

# This script will pull casper-node software and associated files required to run or upgrade
# casper-node.

if [ -z "$1" ]; then
  echo "Error: version argument missing"
  echo "Expected argument containing semantic version of casper_node with underscores as 1_0_2."
  exit 1
fi

SEMVER=$1
ETC_PATH="/etc/casper"
BIN_PATH="/var/lib/casper/bin"

if [ ! -d "$ETC_PATH" ]; then
  echo "Error: expected config file location $ETC_PATH not found."
  exit 2
fi

if [ ! -d "$BIN_PATH" ]; then
  echo "Error: expected bin file location $BIN_PATH not found."
  exit 3
fi

ETC_FULL_PATH="$ETC_PATH/$SEMVER"
BIN_FULL_PATH="$BIN_PATH/$SEMVER"

if [ -d "$ETC_FULL_PATH" ]; then
  echo "Error: config version path $ETC_FULL_PATH already exists. Aborting."
  exit 4
fi

if [ -d "$BIN_FULL_PATH" ]; then
  echo "Error: bin version path $BIN_FULL_PATH already exists. Aborting."
  exit 5
fi

# Should work as
# BASE_URL="https://genesis.casperlabs.io/$SEMVER"
BASE_URL="https://s3.us-east-2.amazonaws.com/genesis.casperlabs.io/$SEMVER"
CONFIG_ARCHIVE="config.tar.gz"
CONFIG_URL="$BASE_URL/$CONFIG_ARCHIVE"
BIN_ARCHIVE="bin.tar.gz"
BIN_URL="$BASE_URL/$BIN_ARCHIVE"

cd $ETC_PATH

echo "Downloading $CONFIG_ARCHIVE from $CONFIG_URL"
if curl -JLO --max-time 15 "$CONFIG_URL"; then
  echo "Complete"
else
  echo "Error: unable to pull $CONFIG_ARCHIVE from $CONFIG_URL."
  echo "File probably doesn't exist.  Please verify version used: $SEMVER"
  exit 6
fi
CONFIG_ARCHIVE_PATH="$ETC_PATH/$CONFIG_ARCHIVE"

echo "Downloading $BIN_ARCHIVE from $BIN_URL"
if curl -JLO --max-time 15 "$BIN_URL"; then
  echo "Complete"
else
  echo "Error: unable to pull $BIN_ARCHIVE from $BIN_URL"
  echo "File probably doesn't exist.  Please verify version used: $SEMVER"
  exit 7
fi
BIN_ARCHIVE_PATH="$ETC_PATH/$BIN_ARCHIVE"

echo "Extracting $BIN_ARCHIVE to $BIN_FULL_PATH"
mkdir -p "$BIN_FULL_PATH"
cd "$BIN_FULL_PATH"
tar -xzvf "$BIN_ARCHIVE_PATH" .

echo "Extracting $CONFIG_ARCHIVE to $ETC_FULL_PATH"
mkdir -p "$ETC_FULL_PATH"
cd "$ETC_FULL_PATH"
tar -xzvf "$CONFIG_ARCHIVE_PATH"

echo "Removing $BIN_ARCHIVE_PATH"
rm "$BIN_ARCHIVE_PATH"
echo "Removing $CONFIG_ARCHIVE_PATH"
rm "$CONFIG_ARCHIVE_PATH"

echo "Process Complete."
echo ""
echo "Creating $ETC_FULL_PATH/config.toml by using config_from_example.sh."
cd "$ETC_PATH"
./config_from_example.sh $SEMVER

#!/usr/bin/env bash

set -e

# This script will pull casper-node software and associated files required to run or upgrade
# casper-node.

if [ -z "$1" ]; then
  echo "Error: version argument missing"
  echo "Expected argument containing semantic version of casper_node with underscores as 1_0_2."
  exit 1
fi


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

ETC_FULL_PATH="$ETC_PATH/$1"
BIN_FULL_PATH="$BIN_PATH/$1"

if [ ! -d "$ETC_FULL_PATH" ]; then
  echo "Error: config version path $ETC_FULL_PATH already exists. Aborting."
  exit 4
fi

if [ ! -d "$BIN_FULL_PATH" ]; then
  echo "Error: bin version path $BIN_FULL_PATH already exists. Aborting."
  exit 5
fi


BASE_URL="https://genesis.casperlabs.io/$1"
CONFIG_URL="$BASE_URL/config.tgz"
BIN_URL="$BASE_URL/bin.tgz"

cd /etc/casper

if curl -JLO --max-time 15 "$CONFIG_URL"; then
  echo "Error: unable to pull config files for version $1 from $CONFIG_URL"
  exit 6
fi

if curl -JLO --max-time 15 "$BIN_URL"; then
  echo "Error: unable to pull bin files for version $1 from $BIN_URL"
  exit 7
fi


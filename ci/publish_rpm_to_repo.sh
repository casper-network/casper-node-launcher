#!/bin/bash
# Publishes built RPMs to an s3-backed RPM repo.
# Author - TV

set -e

usage() {
  cat <<EOF
Usage: $(basename "${BASH_SOURCE[0]}") source_dir="arg" target_bucket="arg" 

Uploads rpms from <source_dir> to <target_bucket>.

Required options:

source_dir     Directory containing RPM(s) to upload
target_bucket  S3 bucket name

EOF
  exit
}

function main() {
    local LOCAL_TEMP_DIR

    LOCAL_TEMP_DIR="/tmp/$TARGET_BUCKET"
    # Create temp directory to use
    mkdir -p "$LOCAL_TEMP_DIR"

    # Ensure necessary packages are installed
    dependencies
    # Sync repo locally
    sync_from_bucket_to_local "$LOCAL_TEMP_DIR"
    # Update local repo
    update_repo "$LOCAL_TEMP_DIR"
    # Sync it back
    sync_local_to_bucket "$LOCAL_TEMP_DIR"
    # Cleanup
    rm -rf "$LOCAL_TEMP_DIR"
}

function dependencies() {
    local DEPS

    DEPS=("aws" "createrepo_c")

    for dep in "${DEPS[@]}"; do
        if [ ! $(which ${dep}) ]; then
            echo "please install $dep"
            exit 1
        fi
    done
}

function sync_from_bucket_to_local() {
    local TEMP_DIR=${1}

    aws s3 sync "s3://$TARGET_BUCKET" "$TEMP_DIR"
} 
    
function update_repo() { 
    local TEMP_DIR=${1}

    mkdir -pv "$TEMP_DIR/x86_64/"
    cp -rv ${SOURCE_DIR}/*.rpm "$TEMP_DIR/x86_64/"

    # Use update flag if not first time
    if [ -e "$TEMP_DIR/x86_64/repodata/repomd.xml" ]; then
        createrepo_c -v --update --deltas "$TEMP_DIR/x86_64/"
    else
        createrepo_c -v --deltas "$TEMP_DIR/x86_64/"
    fi
}

function sync_local_to_bucket() {
    local TEMP_DIR=${1}

    aws s3 sync "$TEMP_DIR" "s3://$TARGET_BUCKET"
}

# Entry
unset SOURCE_DIR
unset TARGET_BUCKET

for ARGUMENT in "$@"; do
    KEY=$(echo "$ARGUMENT" | cut -f1 -d=)
    VALUE=$(echo "$ARGUMENT" | cut -f2 -d=)
    case "$KEY" in
        source_dir) SOURCE_DIR=${VALUE} ;;
        target_bucket) TARGET_BUCKET=${VALUE} ;;
        *help) usage ;;
        -h) usage ;;
        *) usage ;;
    esac
done

if [ -z "$SOURCE_DIR" ] || [ -z "$TARGET_BUCKET" ]; then
    usage
fi

main

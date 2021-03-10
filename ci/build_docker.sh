#!/usr/bin/env bash

set -e

N=casperlabs/casper-node-launcher
C=${DRONE_COMMIT_SHA:-$(git rev-parse --short HEAD)}
#git fetch -t
V=$(git describe --tags --always)


set -x
docker build -t $N:$C .
docker tag $N:$C $N:$V
docker tag $N:$C $N:latest
set +x

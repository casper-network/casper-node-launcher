#!/usr/bin/env bash

N=casperlabs/casper-node-launcher
C=${DRONE_COMMIT_SHA:-$(git rev-parse --short HEAD)}
git fetch -t
V=$(git describe --tags --always)

builtin echo $DOCKER_PASSWORD | docker login -u $DOCKER_USERNAME --password-stdin

set -x
docker push $N:$V
docker push $N:latest
set +x

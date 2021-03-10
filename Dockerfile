FROM casperlabs/node-build-u1804
COPY . /src
RUN cd src && cargo build --release

FROM ubuntu:bionic
LABEL vendor=CasperLabs \
      description="This container holds casper-node-launcher and scripts for operation of a node on the Casper Network."

WORKDIR /root/
RUN apt-get update && \
    apt-get install -y --no-install-recommends curl && \
    rm -rf /var/lib/apt/lists/

COPY --from=0 /src/target/release/casper-node-launcher .
COPY ./resources/maintainer_scripts/*.sh /root/

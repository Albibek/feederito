#!/bin/bash
mkdir -p cache

# rust-aws image is built from docker.io/rust/alpine by adding some musl packages for static building:
# apk update
# apk add musl-utils build-base
podman run -ti -v $PWD:/opt/build -v $PWD/cache:/usr/local/cargo/registry:rw -w /opt/build rust-aws /bin/sh -c 'cargo build -p lambda --release --target=x86_64-unknown-linux-musl' && (pushd target/x86_64-unknown-linux-musl/release/; 7z a -mx5 -tzip ~/projects/platform/aws/modules/rss/files/feederito.zip bootstrap; popd)

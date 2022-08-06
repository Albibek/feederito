#!/bin/bash
mkdir -p cache
podman run -ti -v $PWD:/opt/build -v $PWD/cache:/usr/local/cargo/registry:rw -w /opt/build rust-aws /bin/sh -c 'cargo build --release  --target=x86_64-unknown-linux-musl' && (pushd target/x86_64-unknown-linux-musl/release/; 7z a -mx5 -tzip ~/projects/platform/aws/modules/rss/files/feederito.zip bootstrap; popd)

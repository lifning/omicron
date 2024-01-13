#!/bin/bash
set -ex
tools/ci_download_dendrite_openapi
tools/ci_download_dendrite_stub
source tools/dendrite_openapi_version
cd ../dendrite
git fetch
git checkout "$COMMIT"
cargo build --release --features=tofino_stub --bin dpd --bin swadm
cp target/release/{dpd,swadm} ../omicron/out/dendrite-stub/bin/

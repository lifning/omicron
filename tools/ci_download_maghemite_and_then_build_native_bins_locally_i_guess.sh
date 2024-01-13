#!/bin/bash
set -ex
tools/ci_download_maghemite_openapi
tools/ci_download_maghemite_mgd
source tools/maghemite_mg_openapi_version
cd ../maghemite
git fetch
git checkout "$COMMIT"
cargo build --release --no-default-features --bin mgd
cargo build --release --bin mgadm
cp target/release/{mgd,mgadm} ../omicron/out/mgd/root/opt/oxide/mgd/bin/

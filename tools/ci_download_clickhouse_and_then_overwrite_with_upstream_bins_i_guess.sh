#!/bin/bash
export CLICKHOUSE_NO_VERIFY=1
source tools/ci_download_clickhouse
ver=${CIDL_VERSION/v/}
curl "https://packages.clickhouse.com/tgz/stable/clickhouse-common-static-$ver-arm64.tgz" \
  | tar xz -C out/clickhouse --strip-components 3 "clickhouse-common-static-$ver/usr/bin/clickhouse"

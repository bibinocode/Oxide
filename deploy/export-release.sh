#!/bin/bash
# 导出当前版本及运行依赖镜像，制品不包含数据库卷、会话和环境密钥。
set -euo pipefail
release="${1:?请指定镜像版本}"
case "$release" in *[!a-zA-Z0-9._-]*|'') exit 2 ;; esac
release_dir="/opt/oxide/releases/$release"
mkdir -p "$release_dir"
docker save "oxide-api:$release" "oxide-frontend:$release" postgres:17-alpine redis:7-alpine | gzip -1 > "$release_dir/images.tar.gz.tmp"
mv "$release_dir/images.tar.gz.tmp" "$release_dir/images.tar.gz"
cd "$release_dir"
sha256sum images.tar.gz source.tar.gz > SHA256SUMS

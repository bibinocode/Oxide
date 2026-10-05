#!/bin/sh
# 创建可恢复的 PostgreSQL 一致性备份；不包含 Redis 会话和可重新生成的搜索索引。
set -eu
cd /opt/oxide/stack
backup_dir=/opt/oxide/backups
mkdir -p "$backup_dir"
chmod 700 "$backup_dir"
stamp=$(TZ=Asia/Shanghai date +%Y%m%d-%H%M%S)
backup_file="$backup_dir/oxide-$stamp.dump"
umask 077
docker compose exec -T postgres pg_dump -U oxide -d oxide_blog --format=custom --no-owner --no-acl > "$backup_file.tmp"
mv "$backup_file.tmp" "$backup_file"
sha256sum "$backup_file" > "$backup_file.sha256"
printf '%s\n' "$backup_file"

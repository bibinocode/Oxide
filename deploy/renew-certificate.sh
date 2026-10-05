#!/bin/sh
# Certbot 自动续签成功后同步到 1Panel 挂载目录，再校验并重载 OpenResty。
set -eu
case " ${RENEWED_DOMAINS:-} " in
    *' example.com '*) ;;
    *) exit 0 ;;
esac
install -m 644 "$RENEWED_LINEAGE/fullchain.pem" /opt/1panel/www/sites/example.com/ssl/fullchain.pem
install -m 600 "$RENEWED_LINEAGE/privkey.pem" /opt/1panel/www/sites/example.com/ssl/privkey.pem
docker exec "${OPENRESTY_CONTAINER:?请设置实际 OpenResty 容器名称}" nginx -t
docker exec "${OPENRESTY_CONTAINER:?请设置实际 OpenResty 容器名称}" nginx -s reload

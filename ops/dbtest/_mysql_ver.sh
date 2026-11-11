#!/usr/bin/env bash
set -e
docker start pdb-mysql >/dev/null 2>&1 || true
for i in $(seq 1 30); do
  st=$(docker inspect --format '{{.State.Health.Status}}' pdb-mysql 2>/dev/null)
  [ "$st" = 'healthy' ] && break
  sleep 5
done
echo "IMAGE: $(docker inspect pdb-mysql --format '{{.Config.Image}}')"
docker exec pdb-mysql mysql -uroot -proot -N -e 'SELECT VERSION()' 2>/dev/null || echo "version query failed"

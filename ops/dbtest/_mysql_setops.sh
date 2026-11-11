#!/usr/bin/env bash
set -e
docker start pdb-mysql >/dev/null 2>&1 || true
for i in $(seq 1 30); do
  st=$(docker inspect --format '{{.State.Health.Status}}' pdb-mysql 2>/dev/null)
  [ "$st" = 'healthy' ] && break
  sleep 5
done
echo "--- INTERSECT ---"
docker exec pdb-mysql mysql -uroot -proot -h127.0.0.1 -P3306 -N -e "SELECT 1 INTERSECT SELECT 1" 2>&1 | grep -v Warning || true
echo "--- EXCEPT ---"
docker exec pdb-mysql mysql -uroot -proot -h127.0.0.1 -P3306 -N -e "SELECT 1 EXCEPT SELECT 1" 2>&1 | grep -v Warning || true
echo "--- UNION ---"
docker exec pdb-mysql mysql -uroot -proot -h127.0.0.1 -P3306 -N -e "SELECT 1 UNION SELECT 2" 2>&1 | grep -v Warning || true

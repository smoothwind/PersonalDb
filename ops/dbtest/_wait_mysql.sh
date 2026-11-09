#!/usr/bin/env bash
set -e
docker start pdb-mysql >/dev/null 2>&1 || true
for i in $(seq 1 40); do
  st=$(docker inspect --format '{{.State.Health.Status}}' pdb-mysql 2>/dev/null)
  [ "$st" = 'healthy' ] && { echo "healthy after ~$((i*5))s"; break; }
  sleep 5
done
docker inspect --format '{{.State.Health.Status}}' pdb-mysql

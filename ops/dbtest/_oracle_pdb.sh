#!/usr/bin/env bash
# Oracle FREE 容器被强杀重启后的 PDB 修复：mounted 态全开 + SAVE STATE + 强制监听注册。
# 用法：wsl bash /mnt/d/CodeRepos/PersonalDb/ops/dbtest/_oracle_pdb.sh
set -e
docker start pdb-oracle >/dev/null 2>&1 || true
for i in $(seq 1 84); do
  st=$(docker inspect --format '{{.State.Health.Status}}' pdb-oracle 2>/dev/null)
  [ "$st" = 'healthy' ] && break
  sleep 5
done
echo "=== PDB state ==="
docker exec -i pdb-oracle bash -lc 'sqlplus -s / as sysdba' <<'SQL'
SET PAGESIZE 100
SELECT name, open_mode FROM v$pdbs;
ALTER PLUGGABLE DATABASE ALL OPEN;
ALTER PLUGGABLE DATABASE ALL OPEN SAVE STATE;
ALTER SYSTEM REGISTER;
SELECT name, open_mode FROM v$pdbs;
EXIT
SQL
echo "=== listener services ==="
docker exec pdb-oracle bash -lc 'lsnrctl status 2>/dev/null | grep -iE "Service|FREEPDB" | head -12' || true

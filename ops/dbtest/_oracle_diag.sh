#!/usr/bin/env bash
# Oracle 连接诊断/就绪校验：实例可用性 -> PDB 修复（open+register）-> 容器内 sqlplus 验证 FREEPDB1。
# 成功标志：输出含 CONN_OK（宿主侧重试脚本据此放行 cargo）。
set -e
echo "=== container state ==="
docker inspect --format '{{.State.Status}} {{.State.Health.Status}} (StartedAt {{.State.StartedAt}})' pdb-oracle

echo "=== instance + pdb fix ==="
docker exec -i pdb-oracle bash -lc 'sqlplus -s / as sysdba' <<'SQL'
WHENEVER SQLERROR EXIT SQL.SQLCODE
SET PAGESIZE 100
ALTER PLUGGABLE DATABASE ALL OPEN;
ALTER SYSTEM REGISTER;
SELECT name, open_mode FROM v$pdbs;
EXIT
SQL

sleep 3
echo "=== listener services ==="
docker exec pdb-oracle bash -lc 'lsnrctl status 2>/dev/null | grep -i "Service \"" | head -8' || true

echo "=== sqlplus easy-connect test@test:FREEPDB1 (server-side validation) ==="
# -L：连接失败立即退出（不再重试提示），保证 CONN_OK 判定干净
docker exec -i pdb-oracle bash -lc 'sqlplus -s -L test/test@localhost:1521/FREEPDB1' <<'SQL'
SET PAGESIZE 0
SELECT 'CONN_OK ' || sys_context('USERENV','CON_NAME') FROM dual;
EXIT
SQL
echo "=== done ==="

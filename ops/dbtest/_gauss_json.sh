#!/usr/bin/env bash
# openGauss JSON 能力探针：gsql 在 /usr/local/opengauss/bin，需 LD_LIBRARY_PATH（缺 libcjson 报错即环境未带全）。
# 用 heredoc 经 stdin 传 SQL，避开 gsql -c 的多层引号嵌套。
# 用法：wsl bash /mnt/d/CodeRepos/PersonalDb/ops/dbtest/_gauss_json.sh
set -e
docker start pdb-gauss >/dev/null 2>&1 || true
for i in $(seq 1 40); do
  st=$(docker inspect --format '{{.State.Health.Status}}' pdb-gauss 2>/dev/null)
  [ "$st" = 'healthy' ] && break
  sleep 5
done
docker exec -i pdb-gauss bash -c 'export PATH=/usr/local/opengauss/bin:$PATH; export LD_LIBRARY_PATH=/usr/local/opengauss/lib:$LD_LIBRARY_PATH; export PGPASSWORD="Gauss@456"; gsql -d postgres -U gaussdb -P pager=off' <<'SQL'
SELECT version();
SELECT jsonb_agg(x) FROM (VALUES (1),(2)) AS t(x);
SELECT json_agg(x) FROM (VALUES (1),(2)) AS t(x);
SELECT '{"a": 1}'::jsonb ->> 'a' AS arrow;
SQL

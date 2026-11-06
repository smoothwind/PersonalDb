#!/usr/bin/env bash
# =============================================================================
# OceanBase 容器初始化（幂等）
# 用法（Windows 侧）：wsl -d Ubuntu-24.04 -u root -- bash /mnt/d/CodeRepos/PersonalDb/ops/dbtest/ob-init.sh
#
# 职责：
#   1) 内核参数（fs.aio-max-nr 等，WSL2 单内核持久化）
#   2) 等待 bootstrap（真实判据 = root@sys 可连接；start.sh 的 boot success 是无条件打印，不可信）
#   3) 资源腾挪 + 尝试创建 Oracle 兼容模式租户
#      —— OceanBase 社区版（CE）不支持 Oracle 模式（企业版特性，报 1235
#         "Not support oracle mode"），此步在 CE 上自动跳过；企业版上全自动。
#   4) 验证 OBM（MySQL 模式 test 租户，社区版可用的部分）
#
# 就绪后 PersonalDb 连接：
#   OBM：OceanBase / localhost:2881 / root@test / obroot / Schema 留空
#   OBO（仅企业版）：sys@test_oracle / SysOracle_123 / Schema 留空 / service_name=test_oracle
# =============================================================================
set -uo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"

echo "== 1) 内核参数（OB 要求；WSL2 单内核，一次设置全局生效并持久化）=="
sysctl -w fs.aio-max-nr=1048576 >/dev/null
cat > /etc/sysctl.d/99-oceanbase.conf <<'EOF'
fs.aio-max-nr = 1048576
vm.max_map_count = 1048576
net.core.somaxconn = 4096
EOF
echo "[ok] fs.aio-max-nr=$(sysctl -n fs.aio-max-nr)（含 systemd 开机持久化）"

echo "== 2) 等待 bootstrap（最多 ~15 分钟）=="
booted=0
for i in $(seq 1 90); do
  if docker logs pdb-ob 2>&1 | tail -5 | grep -q 'boot success'; then
    echo "[ok] 日志出现 boot success (~$((i*10))s)"; booted=1; break
  fi
  sleep 10
done
[ "$booted" -eq 1 ] || { echo "!! 等待超时："; docker logs pdb-ob 2>&1 | tail -5; exit 1; }

echo "== 3) 等待 sys 租户可连接（真实就绪判据，最多 ~5 分钟）=="
conned=0
for i in $(seq 1 60); do
  if docker exec pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot \
      -e "SELECT 1" >/dev/null 2>&1; then
    echo "[ok] sys 租户可连接 (~$((i*5))s)"; conned=1; break
  fi
  sleep 5
done
[ "$conned" -eq 1 ] || { echo "!! sys 租户连接超时"; exit 1; }

echo "== 4) Oracle 租户（社区版自动跳过，企业版全自动）=="
# 4a) 在线扩容集群内存（unit 内存只增不减：bootstrap 6G 时 test 占 3G，扩到 9G
#     容量 ~8G 可放 sys2+test3+Oracle2=7G；轮询等配置传播）
docker exec pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot -Doceanbase \
  -e "ALTER SYSTEM SET memory_limit = '9G'" || exit 1
memok=0
for i in $(seq 1 24); do
  cap=$(docker exec pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot -Doceanbase -N \
    -e "SELECT ROUND(MEM_CAPACITY/1024/1024/1024,1) FROM GV\$OB_SERVERS LIMIT 1" 2>/dev/null | tr -d '[:space:]')
  if [ -n "$cap" ] && awk "BEGIN{exit !($cap >= 7)}"; then echo "[ok] MEM_CAPACITY=${cap}G"; memok=1; break; fi
  sleep 5
done
[ "$memok" -eq 1 ] || { echo "!! 内存容量未扩到位（当前 ${cap:-?}G）"; exit 1; }
# 4b) 缩编 test 租户 CPU（镜像默认占满 16C；unit 缩编异步生效，轮询等释放）
docker exec pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot -Doceanbase \
  -e "ALTER RESOURCE UNIT test_unit MIN_CPU = 1, MAX_CPU = 1" || exit 1
cpuok=0
for i in $(seq 1 24); do
  asg=$(docker exec pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot -Doceanbase -N \
    -e "SELECT CPU_ASSIGNED FROM GV\$OB_SERVERS LIMIT 1" 2>/dev/null | tr -d '[:space:]')
  if [ -n "$asg" ] && awk "BEGIN{exit !($asg <= 5)}"; then echo "[ok] CPU_ASSIGNED=${asg}"; cpuok=1; break; fi
  sleep 5
done
[ "$cpuok" -eq 1 ] || { echo "!! CPU 配额未释放（当前 ${asg:-?}C）"; exit 1; }
# 4c) 缩编 test 租户日志盘（镜像默认给满剩余配额）
docker exec pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot -Doceanbase \
  -e "ALTER RESOURCE UNIT test_unit LOG_DISK_SIZE = '3G'" || exit 1
logok=0
for i in $(seq 1 24); do
  lasg=$(docker exec pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot -Doceanbase -N \
    -e "SELECT ROUND(LOG_DISK_ASSIGNED/1024/1024/1024,1) FROM GV\$OB_SERVERS LIMIT 1" 2>/dev/null | tr -d '[:space:]')
  if [ -n "$lasg" ] && awk "BEGIN{exit !($lasg <= 7)}"; then echo "[ok] LOG_ASSIGNED=${lasg}G"; logok=1; break; fi
  sleep 5
done
[ "$logok" -eq 1 ] || { echo "!! 日志盘配额未释放（当前 ${lasg:-?}G）"; exit 1; }
# 4d) 建租户（清残留 pdb_unit → 建单 → 建池 → 建租户）
docker exec pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot -Doceanbase \
  -e "DROP RESOURCE UNIT pdb_unit" >/dev/null 2>&1 || true
tenant_err=$(docker exec -i pdb-ob obclient -h127.0.0.1 -P2881 -uroot@sys -psysroot \
  < "$DIR/ob-oracle-tenant.sql" 2>&1)
tenant_rc=$?
if [ "$tenant_rc" -eq 0 ]; then
  echo "== 4e) 等待 Oracle 租户可连接并设密码（企业版路径）=="
  oready=0
  for i in $(seq 1 36); do
    if docker exec pdb-ob obclient -h127.0.0.1 -P2881 -usys@test_oracle -A \
        -e "SELECT 1 FROM DUAL" >/dev/null 2>&1; then
      echo "[ok] 租户可连接 (~$((i*5))s)"; oready=1; break
    fi
    sleep 5
  done
  [ "$oready" -eq 1 ] || { echo "!! Oracle 租户迟迟不可连接"; exit 1; }
  docker exec pdb-ob obclient -h127.0.0.1 -P2881 -usys@test_oracle -A \
    -e "ALTER USER sys IDENTIFIED BY 'SysOracle_123'" || exit 1
  docker exec pdb-ob obclient -h127.0.0.1 -P2881 -usys@test_oracle -pSysOracle_123 -A \
    -e "SELECT 1 FROM DUAL" >/dev/null 2>&1 && echo "[ok] OBO 就绪：sys@test_oracle / SysOracle_123"
else
  if echo "$tenant_err" | grep -qi "not support oracle"; then
    echo "[skip] 本 OceanBase 为社区版：不支持 Oracle 模式租户（企业版特性，错误 1235）。"
    echo "       OBO 测试需企业版实例（如公司 10.8.130.103）；ob-oracle-tenant.sql 留档，企业版容器上重跑本脚本即可。"
  else
    echo "!! 租户创建失败：$tenant_err"; exit 1
  fi
fi

echo "== 5) 验证 OBM（MySQL 模式 test 租户）=="
docker exec pdb-ob obclient -h127.0.0.1 -P2881 -uroot@test -pobroot \
  -e "SHOW DATABASES" >/dev/null 2>&1 && echo "[done] 容器环境就绪：OBM 可用（root@test / obroot）" || {
  echo "!! test 租户验证失败"; exit 1; }

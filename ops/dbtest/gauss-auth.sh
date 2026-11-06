#!/usr/bin/env bash
# =============================================================================
# openGauss 容器认证兼容修复：sha256 → md5（tokio-gaussdb SASL 兼容）
# 现象：tokio-gaussdb 连 openGauss 5.x 默认 sha256 存储报
#       "invalid message length: expected to be at end of iterator for sasl"
# 处理：password_encryption_type=1（md5）+ 重置密码重新生成存储格式。幂等可重跑。
# 注意：容器内工具不在 PATH（/usr/local/opengauss/bin），且 .bashrc 不导出
#       GAUSSHOME——脚本显式设置环境。
# =============================================================================
set -euo pipefail

ENV_INIT='export GAUSSHOME=/usr/local/opengauss; export PATH=$GAUSSHOME/bin:$PATH; export LD_LIBRARY_PATH=$GAUSSHOME/lib:$LD_LIBRARY_PATH'

echo "== gs_guc reload password_encryption_type=1 =="
# 须以数据目录属主 opengauss 运行（root 会被 gs_guc 拒绝）
docker exec -u opengauss pdb-gauss bash -c "$ENV_INIT; gs_guc reload -D /var/lib/opengauss/data -c 'password_encryption_type=1'" \
  | tail -2

sleep 2
echo "== 重置 gaussdb 密码（关闭复用策略，重新生成 md5 存储）=="
docker exec -u opengauss pdb-gauss bash -c "$ENV_INIT; gs_guc reload -D /var/lib/opengauss/data -c 'password_reuse_max=0' -c 'password_reuse_time=0'" \
  | tail -1
sleep 1
printf "ALTER USER gaussdb IDENTIFIED BY 'Gauss@456';\n" \
  | docker exec -i -u opengauss pdb-gauss bash -c "$ENV_INIT; gsql -d postgres -r"

echo "== 便捷验证（新密码可连）=="
printf "SELECT current_user;\n" \
  | docker exec -i pdb-gauss bash -c "$ENV_INIT; PGPASSWORD='Gauss@456' gsql -h 127.0.0.1 -U gaussdb -d postgres -t"

echo "[done] 认证格式已切换，可重跑 gauss_integration"

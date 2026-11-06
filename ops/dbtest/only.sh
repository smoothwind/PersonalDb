#!/usr/bin/env bash
# =============================================================================
# 单库轮测切换：停掉本项目全部容器，只启动指定 profile（数据卷不受影响）
# 用法（Windows 侧）：
#   wsl -d Ubuntu-24.04 -u root -- bash /mnt/d/CodeRepos/PersonalDb/ops/dbtest/only.sh <profile>
#   <profile> = light(MySQL+PG) | ob(OBM) | oracle(ORA) | gauss(GA) | all
# 配套流程：only.sh <x> → 在 PersonalDb 跑该库用例 → only.sh <下一个>
# =============================================================================
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
C="$DIR/docker-compose.yml"
P="${1:?用法: only.sh light|ob|oracle|gauss|all}"

echo "== 停止本项目全部容器（保留数据卷）=="
# 注意：不带 --profile 的 stop 只选中无 profile 的服务（本项目服务全有 profile）
docker compose -f "$C" --profile all stop

echo "== 启动 [$P] =="
docker compose -f "$C" --profile "$P" up -d

echo "== 当前运行 =="
docker compose -f "$C" --profile "$P" ps --format 'table {{.Name}}\t{{.Status}}\t{{.Ports}}'

#!/usr/bin/env bash
# =============================================================================
# PersonalDb WSL Ubuntu 24.04 一键初始化（root 执行）
# 用途：apt/Docker 源国内加速 + docker-ce + Docker Hub 注册表镜像，为
#       ops/dbtest/docker-compose.yml 提供容器运行时。
# 用法（Windows 侧）：
#   wsl -d Ubuntu-24.04 -u root -- bash -c "cd /mnt/d/CodeRepos/PersonalDb/ops/dbtest && bash ubuntu-setup.sh"
# 若提示 systemd 未激活：wsl --shutdown 后重开 Ubuntu 再跑一遍（脚本幂等）。
# =============================================================================
set -euo pipefail

# ---- 1) wsl.conf：启用 systemd（docker 服务依赖）+ 默认 root（跳过 OOBE 交互）----
cat > /etc/wsl.conf <<'EOF'
[boot]
systemd=true

[user]
default=root
EOF

# ---- 2) apt 源切阿里云（noble 为 deb822 格式 /etc/apt/sources.list.d/ubuntu.sources）----
if [ -f /etc/apt/sources.list.d/ubuntu.sources ]; then
  sed -i \
    -e 's|http://archive.ubuntu.com/ubuntu|https://mirrors.aliyun.com/ubuntu/|g' \
    -e 's|http://security.ubuntu.com/ubuntu|https://mirrors.aliyun.com/ubuntu/|g' \
    -e 's|http://cn.archive.ubuntu.com/ubuntu|https://mirrors.aliyun.com/ubuntu/|g' \
    /etc/apt/sources.list.d/ubuntu.sources
  echo "[ok] apt 源已切阿里云"
fi
apt-get update -qq

# ---- 3) docker-ce（阿里云 docker-ce 源， noble）----
mkdir -p /etc/apt/keyrings
curl -fsSL https://mirrors.aliyun.com/docker-ce/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
echo "deb [arch=amd64 signed-by=/etc/apt/keyrings/docker.asc] https://mirrors.aliyun.com/docker-ce/linux/ubuntu noble stable" \
  > /etc/apt/sources.list.d/docker.list
apt-get update -qq
apt-get install -y -qq docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
echo "[ok] docker-ce 安装完成"

# ---- 4) Docker Hub 注册表镜像（公共镜像站会失效，拉不动就换列表）----
mkdir -p /etc/docker
cat > /etc/docker/daemon.json <<'EOF'
{
  "registry-mirrors": [
    "https://docker.m.daocloud.io",
    "https://docker.1ms.run",
    "https://hub.rat.dev"
  ]
}
EOF
echo "[ok] daemon.json 注册表镜像已写入"

# ---- 5) 启动 docker（需 systemd 生效；否则提示重启发行版）----
if [ -d /run/systemd/system ]; then
  systemctl enable --now docker
  docker version --format 'docker: {{.Server.Version}}'
  docker compose version
  echo "[done] 环境就绪"
else
  echo "!! systemd 尚未激活：请在 Windows 执行  wsl --shutdown  后重开 Ubuntu 再跑一遍本脚本"
fi

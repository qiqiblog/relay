#!/usr/bin/env bash
# 卸载 relay-master（服务端）和/或 relay-node（节点）
#
# 用法：
#   bash <(curl -fsSL https://raw.githubusercontent.com/0xUnixIO/relay/main/uninstall.sh)
#   bash <(curl -fsSL ...) --master          # 只卸载服务端
#   bash <(curl -fsSL ...) --node            # 只卸载节点
#   bash <(curl -fsSL ...) --all             # 全部卸载
#   bash <(curl -fsSL ...) --purge           # 卸载并清除所有数据（不询问）
set -euo pipefail

RED='\033[0;31m'; YELLOW='\033[1;33m'; GREEN='\033[0;32m'; NC='\033[0m'
log()  { echo -e "${GREEN}==>${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
die()  { echo -e "${RED}[✗]${NC} $*" >&2; exit 1; }

# ── 参数解析 ─────────────────────────────────────────────────────────────────
TARGET=""   # master | node | all
PURGE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --master) TARGET="master"; shift ;;
    --node)   TARGET="node";   shift ;;
    --all)    TARGET="all";    shift ;;
    --purge)  PURGE=1;         shift ;;
    *) die "未知参数：$1" ;;
  esac
done

# ── root 检查 / 自动提权 ──────────────────────────────────────────────────────
[[ $EUID -eq 0 ]] || {
  tmp="$(mktemp /tmp/relay-uninstall.XXXXXX.sh)"
  case "$0" in
    /dev/fd/*) cat "$0" > "$tmp" ;;
    bash|-bash|sh|-sh) die "请以 root 运行" ;;
    *) cp "$0" "$tmp" ;;
  esac
  chmod +x "$tmp"
  exec sudo bash "$tmp" "$@"
}
case "$0" in /tmp/relay-uninstall.*.sh) trap 'rm -f "$0"' EXIT ;; esac

# ── 检测已安装组件 ────────────────────────────────────────────────────────────
MASTER_INSTALLED=0
NODE_INSTALLED=0
[[ -f /usr/local/bin/relay-master ]] && MASTER_INSTALLED=1
[[ -f /usr/local/bin/relay-node || -d /usr/local/lib/relay-node ]] && NODE_INSTALLED=1

if [[ $MASTER_INSTALLED -eq 0 && $NODE_INSTALLED -eq 0 ]]; then
  log "未检测到任何 relay 组件，无需卸载。"
  exit 0
fi

# ── 交互菜单（未传参时） ──────────────────────────────────────────────────────
if [[ -z "$TARGET" ]]; then
  [[ -t 0 ]] || exec </dev/tty
  echo
  echo "╔══════════════════════════════════════════╗"
  echo "║          relay 卸载工具                  ║"
  echo "╚══════════════════════════════════════════╝"
  echo
  [[ $MASTER_INSTALLED -eq 1 ]] && echo "  检测到：relay-master（服务端）" || echo "  未安装：relay-master"
  [[ $NODE_INSTALLED   -eq 1 ]] && echo "  检测到：relay-node（节点）"    || echo "  未安装：relay-node"
  echo
  echo "  1. 卸载服务端（relay-master）"
  echo "  2. 卸载节点（relay-node）"
  echo "  3. 全部卸载"
  echo "  4. 退出"
  echo
  read -r -p "请选择 [1-4]: " choice
  case "$choice" in
    1) TARGET="master" ;;
    2) TARGET="node"   ;;
    3) TARGET="all"    ;;
    4) exit 0 ;;
    *) die "无效选择" ;;
  esac
fi

# ── 卸载函数 ──────────────────────────────────────────────────────────────────

uninstall_master() {
  log "停止并禁用 relay-master 服务…"
  systemctl disable --now relay-master 2>/dev/null || true
  rm -f /etc/systemd/system/relay-master.service
  rm -f /usr/local/bin/relay-master
  systemctl daemon-reload
  log "已移除 relay-master 二进制和 systemd 单元"

  # 询问是否清除数据
  local do_purge=$PURGE
  if [[ $do_purge -eq 0 ]] && { [[ -t 0 ]] || [[ -e /dev/tty ]]; }; then
    [[ -t 0 ]] || exec </dev/tty
    echo
    warn "以下数据仍保留在磁盘上："
    [[ -f /etc/relay-master/relay-master.env ]]        && warn "  - /etc/relay-master/relay-master.env  （数据库密码 + JWT 密钥）"
    [[ -d /var/lib/relay-master/pki ]]                 && warn "  - /var/lib/relay-master/pki/           （CA + 服务器证书）"
    [[ -f /etc/relay-master/docker-compose.postgres.yml ]] && warn "  - Postgres 容器 relay-postgres + 数据卷 relay-pgdata"
    [[ -f /etc/relay-master/docker-compose.redis.yml ]]    && warn "  - Redis 容器 relay-redis + 数据卷 relay-redisdata"
    echo
    read -r -p "是否一并清除以上内容（不可恢复）？[y/N] " ans
    case "${ans:-N}" in [Yy]*) do_purge=1 ;; esac
  fi

  if [[ $do_purge -eq 1 ]]; then
    if [[ -f /etc/relay-master/docker-compose.postgres.yml ]]; then
      log "停止并删除内置 Postgres 容器 + 数据卷…"
      docker compose -f /etc/relay-master/docker-compose.postgres.yml \
        ${ENV:+--env-file /etc/relay-master/postgres.env} down -v 2>/dev/null || true
    fi
    if [[ -f /etc/relay-master/docker-compose.redis.yml ]]; then
      log "停止并删除内置 Redis 容器 + 数据卷…"
      docker compose -f /etc/relay-master/docker-compose.redis.yml \
        ${ENV:+--env-file /etc/relay-master/redis.env} down -v 2>/dev/null || true
    fi
    rm -rf /etc/relay-master /var/lib/relay-master
    rm -f /etc/caddy/conf.d/relay.caddyfile
    if [[ -f /etc/caddy/Caddyfile ]] && command -v caddy >/dev/null 2>&1; then
      systemctl reload caddy 2>/dev/null || true
    fi
    # 只有 node 也不存在时才删 relay 用户
    if ! [[ -f /usr/local/bin/relay-node || -d /usr/local/lib/relay-node ]]; then
      if id relay >/dev/null 2>&1; then
        log "删除系统用户 relay…"
        userdel relay 2>/dev/null || true
      fi
    fi
    log "relay-master 已完全清除。"
  else
    log "relay-master 服务和二进制已移除，数据文件保留。"
  fi
}

uninstall_node() {
  log "停止并禁用 relay-node 服务…"
  systemctl disable --now relay-node                2>/dev/null || true
  systemctl disable --now relay-node-updater.path   2>/dev/null || true
  systemctl disable --now relay-node-updater.service 2>/dev/null || true
  rm -f /etc/systemd/system/relay-node.service
  rm -f /etc/systemd/system/relay-node-updater.service
  rm -f /etc/systemd/system/relay-node-updater.path
  rm -f /usr/local/bin/relay-node
  rm -rf /usr/local/lib/relay-node
  systemctl daemon-reload
  log "已移除 relay-node 二进制和 systemd 单元"

  local do_purge=$PURGE
  if [[ $do_purge -eq 0 ]] && { [[ -t 0 ]] || [[ -e /dev/tty ]]; }; then
    [[ -t 0 ]] || exec </dev/tty
    echo
    warn "以下数据仍保留在磁盘上："
    [[ -f /etc/relay-node/relay-node.env ]] && warn "  - /etc/relay-node/relay-node.env"
    [[ -d /var/lib/relay-node/pki ]]        && warn "  - /var/lib/relay-node/pki/  （节点证书 + 私钥）"
    echo
    read -r -p "是否一并清除以上内容（不可恢复）？[y/N] " ans
    case "${ans:-N}" in [Yy]*) do_purge=1 ;; esac
  fi

  if [[ $do_purge -eq 1 ]]; then
    rm -rf /etc/relay-node /var/lib/relay-node
    # 只有 master 也不存在时才删 relay 用户
    if ! [[ -f /usr/local/bin/relay-master ]]; then
      if id relay >/dev/null 2>&1; then
        log "删除系统用户 relay…"
        userdel relay 2>/dev/null || true
      fi
    fi
    log "relay-node 已完全清除。"
  else
    log "relay-node 服务和二进制已移除，数据文件保留。"
  fi
}

# ── 执行卸载 ──────────────────────────────────────────────────────────────────
case "$TARGET" in
  master)
    [[ $MASTER_INSTALLED -eq 1 ]] || { warn "relay-master 未安装，跳过。"; exit 0; }
    uninstall_master
    ;;
  node)
    [[ $NODE_INSTALLED -eq 1 ]] || { warn "relay-node 未安装，跳过。"; exit 0; }
    uninstall_node
    ;;
  all)
    [[ $MASTER_INSTALLED -eq 1 ]] && uninstall_master || warn "relay-master 未安装，跳过。"
    [[ $NODE_INSTALLED   -eq 1 ]] && uninstall_node   || warn "relay-node 未安装，跳过。"
    ;;
esac

log "卸载完成。"

# 部署指南

## 生产部署

发布的 Linux 二进制支持 `x86_64` / `aarch64`，master 与 node 均为 native binary + systemd，**无需安装 psql**。

### 1. 安装 master

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/0xUnixIO/relay/main/install.sh)
```

脚本会询问公网地址、是否用内置 docker compose 启动 Postgres、以及（可选）一个 Web 域名。如果给了域名，脚本会自动装好 Caddy + 用 Let's Encrypt 申请证书做反代，Web UI 直接走 `https://<domain>`，master HTTP 仅监听 `127.0.0.1:7080`；不给则保留 `http://<master-ip>:7080`。

> 域名模式下需要 80/443 端口空闲且 DNS A/AAAA 已指向本机。其余（数据库密码、JWT secret）全自动生成。

### 2. 添加 node

在 Web 控制台 **Nodes → New node** 创建后，弹窗会给出安装命令，复制到目标主机执行即可。

> Enrollment token 仅显示一次。丢失后在节点详情页点 **Rotate token** 重新获取。

---

## 升级

```bash
# 升级 master（不会动 env / DB / PKI）
bash <(curl -fsSL https://raw.githubusercontent.com/0xUnixIO/relay/main/install.sh)

# 升级 node
bash <(curl -fsSL https://raw.githubusercontent.com/0xUnixIO/relay/main/install-node.sh)

# 锁定版本
bash <(curl -fsSL https://raw.githubusercontent.com/0xUnixIO/relay/main/install.sh) --version v0.1.3

# 安装预发布版（rc / 测试版）
bash <(curl -fsSL https://raw.githubusercontent.com/0xUnixIO/relay/main/install.sh) --prerelease
bash <(curl -fsSL https://raw.githubusercontent.com/0xUnixIO/relay/main/install-node.sh) --prerelease
```

---

## 卸载

```bash
# 卸载 master（会询问是否清除 /etc/relay-master、PKI、bundled Postgres，默认保留）
bash <(curl -fsSL https://raw.githubusercontent.com/0xUnixIO/relay/main/install.sh) --uninstall

# 卸载 node（会询问是否清除 /etc/relay-node 与 PKI，默认保留）
bash <(curl -fsSL https://raw.githubusercontent.com/0xUnixIO/relay/main/install-node.sh) --uninstall
```

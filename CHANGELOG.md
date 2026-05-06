# Changelog

按 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 风格组织，遵循 [SemVer](https://semver.org/lang/zh-CN/)。

### Unreleased

- feat(upgrade): 节点远程升级 — admin 可在 Web UI 发起 node-only 远程升级，全程托管。`upgrade_jobs` 表 + 5 态状态机（queued → dispatched → accepted → succeeded / failed / timed_out）。master 解析 GitHub releases（5min 缓存）后给 node 下发 `UPGRADE_AGENT` Command（含 amd64+arm64 两个 asset URL + SHA256SUMS URL），node 把请求原子写入 `/var/lib/relay-node/upgrade-request.json`，systemd path unit 触发 root-only 的 `relay-node-updater` 脚本：下载 → 校验 SHA256 → 解压到 `/usr/local/lib/relay-node/relay-node-$TAG/` → 原子 symlink 切换 → 重启 → 失败回滚。完成判定基于新版本心跳（不依赖 status.json，能跨 master 重启）；10min 没收到匹配版本心跳 → timed_out。Heartbeat 新增 `capabilities` 字段，旧节点（< 0.2.0）UI 升级按钮 disabled + tooltip 提示。Master 自身仍需手动维护，Config 页提供版本展示 + 一键复制安装命令。
- perf(master): 节点心跳运行时改为 in-process L1 + Redis L2 + 节流 Postgres。每节点心跳的 PG 写从"每条一次"降为"至多每 30s 一次"（timestamp-guarded UPDATE 防 reconnect 路径回退），`/api/v1/nodes` 列表 / 详情 / `/api/v1/status` 公开页全部走内存 overlay；在线判定语义不变（仍是 `now - last_seen_at < 15s`），`node_availability` 增加每节点每分钟内存去重。`MASTER_REDIS_URL` 缺席时仍正常运行，仅失去跨重启 warmup。
- feat(master): probe 探测结果 5 秒 Redis 缓存（防抖）— `/api/v1/tunnels/:id/probe` 与 `/api/v1/forwards/:id/probe` 命中缓存直接返回，避免反复打 node。鉴权先行，仅缓存全部成功的 payload。`MASTER_REDIS_URL` 留空时退化为无缓存。
- feat(install): 安装脚本可选启动内置 Redis 容器（`relay-redis` + `relay-redisdata` 卷，loopback only + requirepass）；env 文件新增 `MASTER_REDIS_URL`。
- fix(web): 列表页表头改用 `<colgroup>` 显式列宽，修复 `table-fixed` 下 padding 把窄列撑大、视觉上接近等分的问题（Forwards / Tunnels / Users / UserGroups / NodeDetail 全部统一）
- fix(web): 隧道列表 "分配数" 改名 "转发数"，值从 `user_tunnel_count` 改为 `forward_count`；连通性按钮 icon 由 `Wifi` 换为 `Activity`，与 Forwards 探测按钮保持一致
- perf(node): TCP + UDP 双协议同 forward 共享一个 RateLimiter 桶（按 `(forward_id, hop_index)` 缓存于 `Engine.rate_limiters`），避免双协议下实际限速翻倍；spec 改速时自动重建桶，listener 全部停下后 GC 释放
- feat: 节点证书快过期前自动续期（剩余 < 60 天时静默轮换，无需重装）
- feat: 多协议隧道（**BREAKING schema**）— `tunnels.protocol` 单值升级为 `tunnels.protocols TEXT[]`，默认 `[tcp,udp]`；同一 forward 同 hop 同号 `listen_port` 在 TCP+UDP 两个协议上同时占用；`forward_ports` 主键扩展为 `(forward_id, hop_index, protocol)` 并新增 DEFERRABLE 约束触发器保证同 hop 跨协议端口一致；前端 Tunnels 表单改为协议 checkbox 多选，新增 `ProtocolSetBadge` 紧凑渲染 `TCP+UDP`；改 protocols 时若已有 forward 引用 → 422（与改路径同语义）。迁移文件 `0003_tunnel_protocols_multi.sql` 自动跑。

## [Unreleased] — 0.2.0（**BREAKING**）

> 数据库 schema 一刀切重构：把多跳链路抽成可复用的「隧道」（Tunnel）。
> 升级前**强制备份** `relay` 数据库；不提供反向兼容。

### 升级指南（0.1.x → 0.2.0）

1. **备份数据库**：`pg_dump relay > relay-pre-0.2.0.sql`（必做，无回滚通道）
2. **停掉旧 master**：`systemctl stop relay-master`
3. **升级二进制**：跑 `install.sh` 拉 0.2.0；node 端各自 `install-node.sh` 升级（gRPC/proto BREAKING，旧 node 连不上新 master）
4. **启动新 master**：`systemctl start relay-master`，迁移会按顺序自动跑：
   - `route ↔ tunnel` 概念互换：旧 `tunnels` 表（转发规则）→ `rules`，旧 `routes` → `tunnels`
   - 多上游：`rules.upstream_addr TEXT` → `upstream_addrs TEXT[]` + `lb_strategy`
   - 节点对端地址：`nodes.forwarding_addr TEXT` → `server_ips TEXT[]`
5. **检查节点**：node 重连后 master 会自动把 peer IP 写进空的 `server_ips`；多 NAT/专线场景下务必去节点详情页人工核对
6. **API/UI 名词变化**：调用方需把 `/api/v1/tunnels/*` 改为 `/api/v1/rules/*`；命名隧道用 `/api/v1/tunnels/*`（语义反转）
7. **stats 断点**：单跳 → 多跳切换会改 `PbRule.id`，时间线在升级处会出现一条断点，属预期

### M8 — 概念重命名（Route ↔ Tunnel 互换）

- 历史命名混乱终结：原「Tunnel（转发规则）」→ **Rule**；原「Route（多跳模板）」→ **Tunnel**
- 数据库表 `tunnels` → `rules`、`routes` → `tunnels`、`route_hops` → `tunnel_hops`、`rules.route_id` → `rules.tunnel_id`
- proto: `Tunnel`/`TunnelStats` → `Rule`/`RuleStats`；命令 `RESTART_TUNNEL`/`DRAIN_TUNNEL` → `RESTART_RULE`/`DRAIN_RULE`
- REST: `/api/v1/tunnels/*` 现指命名隧道（旧 routes 资源族）；新增 `/api/v1/rules/*` 取代旧的 `/api/v1/tunnels/*`
- Web UI 导航：`路径` → `隧道`，`转发` 仍指规则；页面文件 `Tunnels.tsx`/`Routes.tsx` → `Rules.tsx`/`Tunnels.tsx`

### M9 — 节点可达地址重构（`forwarding_addr` → `server_ips`）

- **BREAKING**：`nodes.forwarding_addr TEXT` 重命名为 `server_ips TEXT[] NOT NULL DEFAULT '{}'`，迁移自动把旧值包成单元素数组
- 多跳生成下一跳 upstream 时取 `server_ips[0]`；数组为空时与之前 `forwarding_addr=NULL` 行为等价（产出 `0.0.0.0:0`，中转 ACL fail-closed 为 `0.0.0.0/32`）
- **首次连接自动检测**：节点首次 mTLS 接入 `Channel` RPC 时，若 `server_ips` 为空，master 自动把 `TcpConnectInfo::remote_addr()` 的 IP 写入数组（仅一次，不会覆盖用户管理的值）
- REST：`PUT /api/v1/nodes/{id}` 字段 `forwarding_addr?: string` → `server_ips?: string[]`
- Web UI：节点详情页改为可编辑数组（添加/删除/上下移），第一项标记「主」徽章；隧道编辑器缺地址提示文案与跳转链接更新

### M6 — 多跳级联转发

- **多跳 tunnel**：单条规则可定义 `a → b → c` 任意长度链路，master 自动为中间跳分配 listen_port、为中转/出口跳的 ACL 注入上一跳 server_ips[0] 白名单
- 节点 `server_ips` 修改自动触发所有下游路径的 ACL 重编译 + 配置推送
- Web UI tunnel 表单改为多节点链路编辑器（拖拽排序、角色徽章、缺转发地址实时校验）

### M7 — 路径与转发规则解耦

- 新增 `/api/v1/routes` 资源族：路径是命名的多跳模板（`日美 = [tokyo, sf, la]`），多条转发规则可复用同一路径
- 改路径 hops 自动 reconcile 所有引用规则的端口分配；端口尽量复用，入口端口冲突时 fail-closed 422 + 返回阻塞规则列表
- `POST /api/v1/tunnels` 支持 `route_id` 字段绑定命名路径；不传则按 `hops` 创建隐式（per-rule）路径
- 旧表 `tunnels` → `rules`、新增 `routes` / `route_hops` / `rule_hop_ports` 三张表；旧数据迁移为 `imported-{name}` 独立路径（用户后续可在 UI 手动合并）
- Web UI：`/routes` 路径管理页 + 转发表单「路径来源」切换 + 列表展示所属命名路径徽章

### Breaking

- `GET/POST/PUT/DELETE /api/v1/tunnels/*` 响应/请求体的 `node_id` 字段移除（由 `hops[0].node_id` 派生）
- `tunnels` 表结构变更，无向后兼容；升级前必须备份
- 单跳 ↔ 多跳切换会改变 `PbTunnel.id`，导致 stats 时间线在升级处出现断点

## [0.1.3] — 2026-04-26

### 新增
- **Linux 零拷贝转发**：TCP 路径在 Linux 上改用 `splice(2)` + 内核管道缓冲，避免用户态 read/write 拷贝；非 Linux 平台保留 buf-copy 路径。每方向字节计数器仍准确（splice 返回实际移动字节数）
- **上游延迟探测**：master 通过现有 mTLS gRPC 流推 `ProbeRequest`，node TCP-connect 计时回 `ProbeResult`；REST `POST /api/v1/tunnels/:id/probe`；Web UI 在 tunnel 行加「Test」按钮 + Latency 列（成功显示 ms，失败显示 failed + 错误 tooltip）
- **可选 Caddy + Let's Encrypt**：`install.sh` 交互式询问 Web UI 域名；填写后从 cloudsmith apt 仓库装 Caddy，写 `/etc/caddy/conf.d/relay.caddyfile` 反代到 `127.0.0.1:7080`，自动签 LE 证书；不动用户已有的 Caddyfile，与其他站点共存
- **Web 显示版本号**：侧边栏头部显示 master 版本（来自 `/api/v1/server-info` 的 `version` 字段）；节点列表/详情显示 node 版本（来自心跳）
- **节点详情页增强**：每张指标卡片标题旁内联当前数值；新增「Network」卡片，聚合所有 tunnel 的 in/out 速率显示双 sparkline
- **Tunnel 创建/编辑 UX**：Listen 字段从地址输入改为纯端口数字 + 骰子图标（10000-59999 随机端口）+ 同 node 端口冲突即时校验；Upstream 仍是完整 host:port

### 改进
- `install.sh` 取消了未使用的 `--no-prompt` 参数（始终基于 TTY + env 占位符判定是否进交互）
- `install*.sh` 支持 `--prerelease` 拉 `prerelease: true` 的最新预发布版本
- GitHub Actions 升级 `actions/upload-artifact@v7` / `actions/download-artifact@v8`（Node 24）

### 重命名
- 项目从 `relay` 改名为 `relay`，所有 crate 名 / 二进制名 / 安装路径 / env 变量统一为 `relay-*`、`/etc/relay-master`、`/var/lib/relay-{master,node}` 等
- GitHub org 从 `unix-ai` 改为 `relayos`，仓库地址 `0xUnixIO/relay`

## [0.1.2] — 2026-04-26

### 新增
- `relay-master db init` 子命令：连超级用户 DSN，幂等创建 `relay` 角色 + 数据库（无需安装 `psql`）
- `relay-master db migrate` 子命令：手动跑一次内嵌迁移（`serve` 启动时仍会自动跑）
- `deploy/docker-compose.postgres.yml`：一份开箱即用的 Postgres 16 compose 文件，绑 127.0.0.1:5432，命名卷 `relay-pgdata`
- `install.sh` 改为交互式：检测到 TTY 且 env 还是占位值时，问公网地址 + 是否起 docker compose 的 Postgres，其余密码 / JWT 全自动 `openssl rand`，跑完 `db init` 直接 `systemctl enable --now`。`--no-prompt` 回退到旧行为

### 改进
- README 部署一节重写：master 安装从 4 步压缩成 1 条 curl + 2 个交互回答
- `install.sh` 注释引导更新到新流程，不再要求宿主机有 `psql`

### 安全
- `db init` 通过服务端 `format(%I, %L)` 让 Postgres 自己引用标识符 / 字面量，避免手写转义

## [0.1.1] — 2026-04-25

### M4 — mTLS（**BREAKING**）

主控 ↔ 节点的所有 gRPC 通信切换为双向 TLS。原本 v0.1.0 通过应用层 `enrollment_token` + `session_token` 鉴权，本次替换为 master 内置 CA + 客户端证书 + 指纹绑定。

> ⚠️ 一刀切升级：v0.1.0 节点无法连接 v0.1.1 master。每个节点都需要在 Web UI 上 Rotate token 后重新跑 `install-node.sh ... --reenroll`。

#### 新增
- master 启动时自动管理 CA（10y）+ server cert（5y）；SAN 改了自动重签（`MASTER_PUBLIC_ADDR`）
- 新 RPC `Enroll(node_id, enrollment_token, csr_pem)` 在独立 TLS 端口 `:7444`：原子消费 token、用 CA 签 CSR、绑定 cert 指纹/serial/not_after 到 `nodes` 行
- master gRPC `:7443` 强制 client cert，节点身份取自 cert SAN，握手后比对 DB 指纹
- `force_kick(node_id)`：删除节点 / Rotate token 时主动断开活跃连接
- 节点冷启动 enroll：本地生成 keypair → CSR → Enroll RPC → 原子落盘 0600
- install-node.sh：`--ca-cert <base64>` 把 master CA 在安装时预置（避免 TOFU）；`--reenroll` 擦旧 PKI
- `GET /api/v1/server-info`：暴露 public host / 端口 / CA PEM / CA b64，前端不再用 `window.location` 凑
- `POST /api/v1/nodes/:id/rotate-token`：换 token 同时清 cert 绑定 + 踢通道
- Web UI 节点详情页：证书指纹 / serial / not_after / enrolled_at + Rotate token 按钮（弹出 `--reenroll` 命令）

#### 移除
- 原 `Register` RPC + `session_token` 字段 + `x-node-id` / `x-session-token` 元数据流程

#### 数据库迁移
- `20260425000004_node_certs.sql`：`nodes` 加 `cert_fingerprint` / `cert_serial` / `cert_not_after`；`enrollment_token` 改 nullable；id 加 `^[a-z0-9][a-z0-9._-]{0,62}$` CHECK
- `20260425000005_drop_session_token.sql`：删除 `session_token` 字段

#### 升级步骤（v0.1.0 → v0.1.1）
1. 升级 master 包并重启（DB 迁移自动跑；CA + server cert 自动生成）
2. 在 Web UI 上对每个 node 点 **Rotate token**，复制弹窗里的命令
3. 在节点机器上跑该命令（含 `--reenroll`）

## [0.1.0] — 2026-04-XX

首个公开版本：M0 脚手架 / M1 控制面 / M2 转发引擎 / M3 Web 控制台 / M3.5 sparkline + CI 发布管线。详见 [ROADMAP](./ROADMAP.md)。

# 路线图

按里程碑组织，每个里程碑产出可用的增量。

## M0 — 项目脚手架 ✅

- [x] Cargo workspace（`proto` / `common` / `master` / `node`）
- [x] 纯 Bun 前端（Bun.serve 开发服务 + Bun.build 打包 + Tailwind v4 + shadcn/ui）
- [x] gRPC `.proto` 定义 + tonic 编译
- [x] 基础 tracing 日志、CLI 参数、`.env` 加载
- [x] `Makefile`（`make dev` 一键拉起 master / node / web）
- [x] `.gitignore`、README、ROADMAP（中文）

## M1 — 控制面核心 ✅

- [x] PostgreSQL schema + sqlx migrations（`users` / `nodes` / `tunnels` / `audit_log` + `updated_at` 触发器）
- [x] master 配置加载（`MASTER_DATABASE_URL` / `MASTER_JWT_SECRET` 等）
- [x] master REST API
  - [x] 鉴权：`/auth/bootstrap`（首次创建 admin）、`/auth/login`（argon2 + JWT）
  - [x] 节点 CRUD：`POST /api/v1/nodes`（入库并签发一次性 enrollment_token）、list、delete
  - [x] 转发规则 CRUD：list、create、update、delete + 协议白名单
- [x] master gRPC 服务
  - [x] `Register`：校验 enrollment_token，写入 `enrolled_at` / hostname / version，签发 `session_token`
  - [x] `Channel` 双向流：基于 `x-node-id` + `x-session-token` 校验；心跳写 `last_seen_at` + `last_heartbeat` JSONB
  - [x] `SyncConfig`：连接打开即推送当前 tunnel 集，CRUD 后增量推送（latest-wins watch）
- [x] node 代理
  - [x] 启动后向 master 注册（带 enrollment_token）
  - [x] 周期心跳上报（含 `x-node-id` + `x-session-token` metadata）
  - [x] 接收并应用配置 diff，回 ConfigAck（master 写 `last_applied_version`）
  - [ ] 流量统计（M2.5）

## M2 — 转发引擎 ✅ 基础完成

- [x] TCP 转发器（每条 tunnel 一个 listener → upstream，`copy_bidirectional`）
- [x] **Linux 零拷贝**（`splice(2)` + 内核管道缓冲，非 Linux 自动回退到 buf-copy）
- [x] 最大连接数限制（per-tunnel 信号量）
- [x] UDP 转发器（NAT 风格 per-client `connect()` 上行 socket + 60s 空闲清理 + 1024 会话上限）
- [x] 配置热更新（diff-based：未变 tunnel 保持运行；变更/移除 → cancel token；新增 → 启动）
- [x] 配置推送严格单调版本（`nodes.tunnels_version` 与 CRUD 同事务 bump；node 上报 `last_applied_version`）
- [x] 连接级 conn_id 防止重连互踢（旧连接关闭只在 conn_id 仍属自己时清理 registry）
- [x] 流量计数（in/out 字节、活跃连接数）通过 gRPC 周期上报，落 `tunnel_stats` 时序表
- [x] **上游延迟探测**：master 通过现有 mTLS gRPC 流推 ProbeRequest，node TCP-connect 计时回 ProbeResult；Web UI 在 tunnel 行提供 Test 按钮
- [x] 限速（令牌桶）— `speed_limit_kbps` per user_tunnel：master snapshot 仅入口 hop 注入，node 端 `RateLimiter` 令牌桶（per-forward 总速率，1s 突发上限），TCP（splice + buf-copy）/ UDP 双向 consume；**双协议同 forward 共享同一桶**（按 `(forward_id, hop_index)` 缓存于 `Engine.rate_limiters`）合并计费
- [ ] CIDR 黑白名单（M2.5 后续）

## M3 — Web 控制台 ✅ 基础完成

- [x] shadcn 主题 + 整体布局（侧边栏 + 顶栏）+ 登录页
- [x] 登录页：检测首次部署引导 (`/auth/status`)，admin/admin 一次性 bootstrap，JWT 存 localStorage
- [x] 受保护路由：路由守卫 + 401 自动重定向 /login
- [x] 节点列表页：状态徽章（在线/离线，15s 阈值）、版本、最后心跳、标签、删除
- [x] 节点创建对话框：一次性 enrollment_token 显式展示 + 复制 + 命令片段
- [x] 转发规则页：CRUD + enable/disable Switch + 协议/Node 下拉
- [x] master REST 加 JWT middleware 保护 `/api/v1/*`（除 `/auth/*` `/health`）+ CORS
- [x] 节点详情页：心跳/隧道历史指标（5s 间隔，10 分钟环形缓冲，SVG sparkline）
- [x] 节点详情页：流量曲线（in/out bytes、活跃连接数 sparkline，复用 stats 时序）
- [x] **节点详情页**：每张卡片标题旁的实时数值 + 网速（Network in/out per-tunnel 聚合）卡片
- [x] **侧边栏显示 master 版本**（`/api/v1/server-info` 返回 `version`），节点列表/详情显示 node 版本（来自心跳）
- [x] **Tunnel 行 Test 按钮**：触发上游延迟探测，列内显示 ms / failed
- [x] **Tunnel listen 字段 UX**：仅输入端口 + 骰子随机端口（10000-59999）+ 同 node 端口冲突即时校验
- [ ] 实时日志 tail / WebSocket 推送（M4 第二阶段）
- [ ] 批量操作 / RBAC（M4 第二阶段）

## M4 — 运维与安全 ✅ mTLS 完成（v0.1.1）

> 实施前已经过 rubber-duck 评审，下面是修订后的计划，已纳入 8 项关键安全修正。

### M4.1 — PKI 基建（master 内置 CA） ✅
- [x] PKI 目录改用 `/var/lib/relay-master/pki`（systemd `StateDirectory=relay-master`），避开 `ProtectSystem=strict` 的只读 `/etc`
- [x] master 启动时调用 `Pki::ensure(dir, public_addrs)`：
  - `ca.crt|ca.key` **均不存在**才视为 cold start 自动生成（rcgen，10y），否则任一缺失即 fatal 退出
  - `server.crt|server.key` 校验：CA 链有效 + 未过期 + 公私钥匹配 + SAN 与 `MASTER_PUBLIC_ADDR` 一致；任一不满足触发重签（5y）
  - 文件 mode 0600，`relay` 用户拥有
- [x] 启动时若 `MASTER_PUBLIC_ADDR` 未配置，**直接拒绝启动**（不要悄悄用 hostname 凑）

### M4.2 — Node enrollment（CSR 签发） ✅
- [x] 新 RPC `Enroll(node_id, enrollment_token, csr_pem) → (node_cert, ca_cert)`，独立 TLS 端口 `:7444`（仍是 TLS，使用 CA 签的 server cert，不要求 client cert）
- [x] **enrollment_token 原子消费**：`UPDATE nodes SET enrollment_token = NULL WHERE id=$1 AND enrollment_token=$2 RETURNING ...`，仅 RETURNING 命中行才签发
- [x] CSR 处理：忽略 subject / SAN 字段，**只取公钥**；服务端用已知 `node_id` 与公钥重新组装签名
- [x] `node_id` 严格校验 `^[a-z0-9][a-z0-9._-]{0,62}$`
- [x] 证书设置：CA `BasicConstraints=CA:TRUE,pathlen:0`，server cert `EKU=serverAuth`，node cert `EKU=clientAuth`，KU 相应限制
- [x] node cert 默认 365 天有效（不是 5 年），便于将来加 rotate
- [x] 签发成功后 `nodes` 表写入 `cert_fingerprint`、`cert_serial`、`cert_not_after`
- [x] 删除原 `Register` RPC + `session_token` 字段

### M4.3 — gRPC 全切 mTLS ✅
- [x] master gRPC server (`:7443`) 强制 client cert 验证，从 cert SAN 取 `node_id`
- [x] Channel 握手后比对 `nodes.cert_fingerprint`，不匹配（吊销/已替换）则拒绝并 close stream
- [x] registry 加 `force_kick(node_id)`：rotate / delete 时主动断开活跃连接
- [x] node gRPC client 加载 `/var/lib/relay-node/pki/{node.crt,node.key,ca.crt}`，`server_name` = `MASTER_PUBLIC_ADDR` 的 host 部分
- [x] **一刀切**：v0.1.1 后只接 mTLS。v0.1.0 旧 node 会在 TLS 握手层失败，文档写明「升级要求重新 enroll」

### M4.4 — install-node.sh + node 启动 enroll ✅
- [x] PKI 目录 `/var/lib/relay-node/pki`（`StateDirectory=relay-node`）
- [x] **CA 信任 bootstrap**：Web UI 生成的安装命令直接内嵌 base64 编码的 CA cert（`--ca-cert <base64>`），install-node.sh 落盘后用此 CA 校验 Enroll 端口的 TLS 证书 —— 不做 TOFU，不接受 insecure
- [x] 加 `--reenroll` 标志：擦掉旧 PKI + 覆盖 env，便于 rotate token / 换 master 场景
- [x] node 启动逻辑：
  - 已有 cert/key/ca → mTLS 直连 `:7443`
  - 否则：要求 `NODE_TOKEN` + 已 bootstrap 的 CA，本地生成 keypair → CSR → Enroll RPC → 临时文件 + fsync + rename 原子落盘 → 再 mTLS 连接
- [x] 端到端先打通再删除 `Register`（M4.3 一刀切）

### M4.5 — Web UI 增强 ✅
- [x] 后端新增 `GET /api/v1/server-info`：返回 `public_grpc_addr`, `public_enroll_addr`, `ca_cert_pem`，不再让前端用 `window.location.hostname` 凑
- [x] 创建 node 弹窗：安装命令使用 server-info 数据 + 内嵌 CA cert
- [x] 节点详情页：显示证书 CN / 指纹 / `not_after`
- [x] 节点详情页：「Rotate enrollment token」按钮 → 后端发新 token 同时清 `cert_fingerprint` + `force_kick`，弹窗给新安装命令；用户复制后在 node 上 `install-node.sh --reenroll`

### M4 第二阶段（暂不在本里程碑）
- [ ] **Tunnel 自动健康检测** — 单次手动 probe（`POST /tunnels/:id/probe`）已有；待补全：
  - master 周期性自动探测所有启用的 tunnel（可配置间隔，默认 30s）
  - `tunnels` 表新增 `health_status`（ok / degraded / down）+ `last_checked_at` + `last_error`
  - Web UI tunnel 列表展示健康状态徽章，详情页显示最近探测时间与失败原因
  - 可选：状态变化时触发 Webhook 通知
- [ ] 配置变更审计日志（写 `audit_log` 表 + Web UI 列表）
- [ ] RBAC（admin / operator / viewer 三角色）
- [ ] 结构化 JSON 日志 + 模块级日志级别
- [ ] 优雅停机、信号处理、与 supervisor 兼容的退出码
- [ ] 证书自动续签 / CRL（先用 365d 手动 rotate 顶住）

## M5 — 分发与部署 ✅（v0.1.2）

- [x] master / node 原生二进制（Linux x86_64 / aarch64）— 全栈 native，不打 Docker：node 避免 netns/iptables/端口映射等转发开销，master 简化运维栈
- [x] systemd unit + 一键安装脚本：
  - master：`bash <(curl -fsSL …/install.sh)`，自动 sudo 提权、TTY 探测交互式向导、`MASTER_PUBLIC_ADDR` 自动探测（`api.ipify.org` → LAN IP fallback）、自动安装 docker（缺失时询问 `get.docker.com`）、bundled `docker-compose.postgres.yml` 起 Postgres、自动跑 `relay-master db init` + 写 env + `systemctl enable --now`；可选启动 bundled `docker-compose.redis.yml`（`relay-redis` 容器 + `relay-redisdata` 卷，loopback only + requirepass），写 `MASTER_REDIS_URL` 启用 probe 防抖缓存
  - master：**可选 Caddy + Let's Encrypt**——交互式询问 Web UI 域名，自动装 Caddy + 反代 + HTTPS（保留与用户其他站点共存）
  - node：`bash <(curl -fsSL …/install-node.sh)`，相同自提权与 enroll 流程，命令由 Web UI 自动生成（含 base64 CA cert）
  - 卸载：`--uninstall` 交互式询问是否清空数据/容器卷/用户（默认保留）
- [x] `relay-master db init|migrate` 子命令：用 `SELECT format('CREATE ROLE %I LOGIN PASSWORD %L', …)` 让 Postgres 自己引号转义，避免 DDL 注入
- [x] GitHub Actions 发布流水线（tag 触发，linux x86_64 + aarch64 tarball + SHA256SUMS + web bundle + GitHub Release）

## M6 — 多跳级联转发 ✅

- [x] DB schema：`tunnel_hops`（`tunnel_id`, `hop_index`, `node_id`, `listen_port`）+ `tunnels` 去 `node_id`/`listen_port` 直接列；index 0 = 入口
- [x] master 编排器：建/改 tunnel 时按链路顺序在每个 node 上分配 listen_port；中转 hop 的 upstream = 下一跳 node 的 `server_ips[0]:listen_port`；出口 hop 的 upstream = 用户配置目标
- [x] Hop ACL：中转/出口 hop 的 allow_cidrs 自动加入「上一跳 node 的 server_ips[0] 主机」白名单
- [x] node `server_ips` 改变 → 找所有「下一跳是该 node」的 tunnel → bump 受影响 node 版本 → 推送
- [x] Web UI：tunnel 表单改为多节点链路（拖拽排序 + 角色徽章 入口/中转/出口 + 缺转发地址校验）；列表按 `a → b → c` 展示链路
- [x] 配置推送顺序：受影响节点按下游优先排序，避免中间态丢包

## M7 — 路径（Route）与转发规则（Rule）解耦 🚧

> 把多跳链路抽成可复用的命名实体「路径」（Route），转发规则只引用 `route_id`。
> v0.2.0 一刀切 breaking，详见升级指南。

- [x] **M7.1** 数据模型 + 迁移
  - 新增 `routes` / `route_hops` / `rule_hop_ports` 表；`tunnels` 去 `node_id`/`listen_port`、加 `route_id` NOT NULL FK
  - `rule_hop_ports → route_hops` 复合 FK ON DELETE RESTRICT，机械保证路径变更必须经过 reconcile
  - 旧 tunnel 自动迁移为 `imported-{name}` 独立路径
- [x] **M7.2** Master 编排器
  - `routes` 模块管 CRUD + reconcile 算法（端口复用 + 入口端口冲突 fail-closed）
  - `allocate_hop` 改用 `ON CONFLICT ON CONSTRAINT … DO NOTHING RETURNING` 避免事务被 UNIQUE 违反终止
  - 受影响 node 的 advisory lock + 在事务内构建 reservation map 检测同批冲突
  - 隐式路径前缀 `__implicit-` 区分自动生成 vs 用户命名；删除 tunnel 时仅清隐式且未被引用的路径
- [x] **M7.3** HTTP API
  - `/api/v1/routes` 全套（list / create / get / put / delete / preview-update）
  - `create_tunnel` 支持 `route_id` 字段：传入则绑定命名路径，否则按原 `hops` 建隐式路径
- [x] **M7.4** Web UI
  - 新增 `/routes` 页面：列表（含引用规则数）、创建/编辑（hop 多选 + 排序）、preview-update 显示影响面、删除前 refcount 守卫
  - 转发表单加「路径来源」开关：自定义跳点 vs 选择已有路径；命名路径下隐藏 hops UI、入口节点取自路径首跳
  - 转发列表 链路 列对命名路径加 `路径：xxx` 徽章
- [ ] **M7.5** 测试 + 文档
  - orchestrator 单测（端口复用、reconcile、入口冲突、共享路径并发改动）
  - e2e：路径 → 2 条规则共用 → 改路径加一跳 → 验证两条规则都重下发
  - 升级指南 v0.1.x → v0.2.0（备份提示 + breaking 说明 + stats 时间线断裂提示）

## M8 — 概念重命名（Route ↔ Tunnel 互换）✅

> 历史命名混乱终结：把原「Tunnel（转发规则）」改名为 **Rule**，把原「Route（多跳模板）」改名为 **Tunnel**。
> 一次性 breaking 重命名，覆盖 DB / proto / REST / 前端。

- [x] DB 表 `tunnels` → `rules`、`routes` → `tunnels`、`route_hops` → `tunnel_hops`；列 `route_id` → `tunnel_id`
- [x] proto 类型 `Tunnel` → `Rule`、`TunnelStats` → `RuleStats`；命令 `RESTART_TUNNEL`/`DRAIN_TUNNEL` → `RESTART_RULE`/`DRAIN_RULE`
- [x] REST：`/api/v1/rules/*` 管转发规则、`/api/v1/tunnels/*` 管命名多跳隧道
- [x] Web UI：`路径` 导航改为 `隧道`；页面文件 `Tunnels.tsx` / `Routes.tsx` → `Rules.tsx` / `Tunnels.tsx`

## M9 — 对齐 flvx 的二层模型（Tunnel 模板 + Forward 实例）✅ 主体完成

> 把 M8 的扁平 Rule 模型重构为 flvx 风格的两层结构：admin 创建 **Tunnel（路径模板）**
> + 分配 `user_tunnels` 给用户（带流量/到期配额），用户在配额内创建 **Forward（实际端口转发实例）**。
> 一次性 breaking DB 重写；老节点凭 `protocol_version` 拒绝下发。

- [x] **M9.0** proto `protocol_version` 门禁（M9=2），低版本节点 warn-log 不下发
- [x] **M9.1** Schema + 后端核心
  - 重写 `0001_init.sql`：删 `rules`/`rule_hop_ports`/`tunnel_hops`/`user_quota`；
    加 `tunnels`/`tunnel_hops`(in/chain/out 角色)/`user_tunnels`/`forwards`/
    `forward_ports`/`forward_pause_reasons`
  - `forward_ports` UNIQUE(node_id, protocol, listen_port)，TCP/UDP 同端口可共存
  - `ports.rs` advisory-lock 端口分配器；`pause.rs` 7 种 reason 状态机
  - HTTP：tunnels / user-tunnels / forwards CRUD + pause/resume/redeploy + 批量
  - registry 按 hop 拆 `ForwardConfig`；transit hop /32 ACL 保留
  - grpc `ForwardStats`：hop_index=0 入口才计费；超额内联写 `tunnel_quota_exceeded`
- [x] **M9.2** Node
  - `forward.rs` 接收 `ForwardConfig`，spec PartialEq 含 `deploy_generation`
    → redeploy 强制 listener 重建
  - `ForwardStats` 上报含 `forward_id` + `hop_index`
- [x] **M9.3** Web UI
  - 新增 `/tunnels`（admin 路径模板 CRUD）、`/forwards`（取代 `/rules`）
  - 用户详情新增「分配隧道」抽屉（配额 + 到期）
  - `/me`：用户视角看自己的 user_tunnels 配额进度 + 我的 forwards
- [x] **M9.4** 后台调度器 + 反应式 kick
  - `scheduler.rs` 30s tick：lifecycle reasons 重算 + 用户 expired 翻转 +
    配额 reason 重算 + bump/push 受影响节点
  - `scheduler::kick()` 一次性触发；接到 update_user / update_tunnel /
    update_user_tunnel，admin 改完立即生效
- [ ] **M9.5** 打磨
  - forward / tunnel diagnose 端点
  - sparkline series key 改 `forward_id:hop_index`
  - `last_deploy_error` UI banner + `deploy_failed` reason 自动恢复

## M10 — 每跳多节点负载均衡

> 隧道的任意跳（入口、中转、出口）均可配置多台节点，形成「节点组」。
> 组内节点按策略分摊流量；某台宕机时其余节点自动承接。
>
> 示意：`客户端 → [A1 | A2（入口组）] → [B1 | B2（出口组）]`
>
> 上游节点（A 组）持有下游节点组（B 组）所有成员的地址列表，
> 复用现有 `lb_strategy`（round_robin / primary_backup）选路。

### M10.1 — 数据模型

- [ ] `tunnel_hops` 新增 `weight` 列（默认 1），同一 `hop_index` 允许多行——每行一台节点，共同构成该跳的节点组
- [ ] `forward_ports` 同一 `hop_index` 可有多行（组内每台节点各分配一个端口）
- [ ] `forwards.entry_addrs`（新列，`text[]`）：所有入口节点的 `server_ip:port` 列表，替代单值 `entry_addr`

### M10.2 — 端口分配

- [ ] `allocate_forward_ports` 对每跳节点组内逐台分配端口，组内各节点端口号可独立，不要求相同
- [ ] `probe_and_fix_ports` 对每跳组内所有节点逐一探测修复

### M10.3 — 配置推送

- [ ] registry 构建 `ForwardConfig` 时，`upstream_addrs` 填入**下一跳节点组**所有成员的 `server_ip:port`（组内多地址，复用现有 `lb_strategy` 字段选路）
- [ ] 节点组内任一节点 `server_ips` 变更 → 找所有「下一跳含该节点」的 forward → bump 受影响节点版本 → 推送
- [ ] 入口跳（`hop_index = 0`）各节点的 `upstream_addrs` 都指向下一跳的完整节点组地址列表

### M10.4 — Web UI

- [ ] `HopEditor` 支持同一跳添加多台节点（「+ 同跳节点」按钮），同跳节点以缩进子列表展示，角色标注「入口组 ×2」/「出口组 ×3」等
- [ ] 转发详情展示所有入口地址列表（可逐条复制）
- [ ] 节点组内各节点在线状态独立显示，配置离线降权策略（仅警告 / 自动剔除）

---

## M11 — 进阶能力（候选池）

- [ ] HTTP/HTTPS 反向代理 tunnel（含 SNI 路由）
- [ ] node 上的 SOCKS5 / shadowsocks 风格出口
- [ ] 真·多入/多出 LB/HA（M9 故意未做，避免伪功能）
- [ ] 故障切换分组（多 node 主备）
- [ ] 基于地理位置 / 延迟的路由
- [ ] 自定义协议的插件接口
- [ ] gost / xray 引擎可选切换
- [ ] `tunnel_quota` 月度自动翻页（当前手动重置）
- [x] `speed_limit` 实施（令牌桶，详见 M2 章）
- [ ] 多 master federation
- [x] 多协议隧道（单条 Tunnel 同时支持 TCP + UDP；`tunnels.protocols TEXT[]` 默认 `[tcp,udp]`，同 hop 同号 `listen_port` 在两个协议上同时占用；前端两枚 checkbox + `ProtocolSetBadge` 合并展示；改协议受 forward 占用拦截，与改路径同样语义）

---

### 每个里程碑的「完成」标准

- 功能尽可能配套单元测试或集成测试
- `cargo fmt`、`cargo clippy -D warnings`、`cargo test` 全绿
- 前端 `bun run typecheck`、`bun run build` 全绿
- 文档同步更新（`README.md` + ROADMAP 对应章节）

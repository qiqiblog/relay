# 本地开发

## 目录结构

```
relay/
├── Cargo.toml                # workspace
├── crates/
│   ├── proto/                # gRPC .proto 与生成代码
│   ├── common/               # 共享类型 / 工具
│   ├── master/               # 控制面二进制
│   └── node/                 # 转发节点 agent
├── web/                      # React + shadcn 控制台（纯 Bun）
├── deploy/systemd/           # systemd unit 与 env 示例
├── install.sh                # master 一键安装
├── install-node.sh           # node 一键安装
└── ROADMAP.md
```

## 前置依赖

- Rust ≥ 1.80（已安装 `protoc`）
- Bun ≥ 1.1
- PostgreSQL ≥ 14

## 一键启动

```bash
cp .env.example .env
make dev          # 同时拉起 master + node + web
```

其它常用 target：`make build`、`make check`、`make fmt`、`make lint`、`make test`。运行 `make help` 查看全部。

## Git hooks（推荐）

首次 clone 后跑一次：

```bash
make hooks
```

会把 `core.hooksPath` 指到仓内 `.githooks/`：

- **pre-commit**（秒级）：触及 `*.rs` 时跑 `cargo fmt --check`；触及 `web/` 时跑 `bun x tsc --noEmit`
- **pre-push**（数十秒）：跑 `cargo clippy --all -- -D warnings` + `bun run build`

紧急情况可用 `--no-verify` 跳过。

## 手动启动

```bash
cargo run -p relay-master
cargo run -p relay-node
cd web && bun install && bun run dev   # 监听 :5173，/api 反代到 master
```

## 前端工程说明

`web/` 目录只用 Bun，无 Vite，无 PostCSS：

- `bun run dev` — 开发服务（watch + HMR，不产生 `dist/`）
- `bun run build` — 通过 `build.ts` 调用 `Bun.build`，产出 `dist/` 静态产物
- Tailwind v4（CSS-first，配置全部在 `src/index.css` 的 `@theme` 中）

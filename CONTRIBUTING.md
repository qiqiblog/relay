# 贡献指南

感谢你考虑为 relay 做贡献！

## 前置依赖

- Rust ≥ 1.80（需安装 `protoc`）
- Bun ≥ 1.1
- PostgreSQL ≥ 14（或用 `docker compose` 启动，见下）

## 本地开发

```bash
git clone https://github.com/0xUnixIO/relay.git
cd relay
cp .env.example .env
make hooks   # 安装 git hooks（推荐）
make dev     # 同时拉起 master + node + web
```

常用 make target：

| 命令 | 说明 |
|---|---|
| `make dev` | 一键启动 master + node + web |
| `make build` | 构建所有二进制 + 前端 |
| `make check` | 等同 CI：fmt + clippy + test + typecheck |
| `make fmt` | cargo fmt + 前端格式化 |
| `make lint` | cargo clippy |
| `make test` | cargo test |

完整说明见 [docs/development.md](./docs/development.md)。

## 提交 PR

1. Fork 仓库，基于 `main` 创建分支（`feat/xxx`、`fix/xxx`）
2. 保证 `make check` 全绿后再推送
3. Commit message 遵循 [Conventional Commits](https://www.conventionalcommits.org/)（`feat:` / `fix:` / `docs:` 等）
4. 在 PR 描述中说明改动背景和测试方式

## 代码风格

- **Rust**：`cargo fmt`（pre-commit hook 自动检查），lint 跑 `cargo clippy -- -D warnings`
- **TypeScript**：`bun x tsc --noEmit` 无错误；pre-commit hook 会自动触发
- 新功能尽量附带测试

## 报告问题

使用 GitHub Issues，尽量填写复现步骤和版本信息。  
安全漏洞请参阅 [SECURITY.md](./SECURITY.md)。

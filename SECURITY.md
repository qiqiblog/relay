# Security Policy

## Supported Versions

| Version | Supported |
|---|---|
| Latest release | ✅ |
| Older releases | ❌ |

我们只对最新发布版本提供安全修复。

## Reporting a Vulnerability

**请勿通过公开 Issue 报告安全漏洞。**

请发邮件至 **security@relay.dev**，邮件包含：

- 漏洞描述与影响范围
- 复现步骤（版本、环境、PoC）
- 你认为合理的修复建议（可选）

我们会在 **72 小时内**确认收到，并在评估后与你协商披露时间线（一般不超过 90 天）。

## Scope

以下属于漏洞范围：

- master REST API / gRPC 未授权访问
- enrollment token / session token 泄漏或绕过
- 转发规则导致的 SSRF / 横向移动
- PKI 证书校验缺陷

以下**不在**漏洞范围：

- 已知依赖项的 CVE（请提 Issue 跟踪升级）
- 需要宿主机 root 权限才能触发的问题
- 拒绝服务（DoS）攻击

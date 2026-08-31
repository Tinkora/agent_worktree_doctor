# Agent Worktree Doctor

[English](README.md)

Agent Worktree Doctor 是一个本地、离线、只读的 Git worktree 拓扑与元数据诊断
CLI。首个版本聚焦 `.git` 正向指针、管理目录反向指针、`commondir`、Git 登记的
`prunable` 与 `locked` 状态、worktree 配置风险，以及可选的浅层 submodule 和已跟踪
状态检查。

项目仍在积极开发。v0.1 契约见[产品规格](docs/PRODUCT_SPEC.zh-CN.md)；在自动化中
依赖某个具体 finding 前，请核对当前版本说明。

## 安装与使用

从当前检出构建：

```bash
cargo install --path .
agent_worktree_doctor audit /path/to/worktree
agent_worktree_doctor audit repo_a repo_b --format json
agent_worktree_doctor audit repo --include-submodules --check-tracked-status
```

预期命令形式：

```text
agent_worktree_doctor audit <PATH>... [--include-submodules]
  [--check-tracked-status] [--format text|json|sarif]
```

至少需要一个路径。退出状态 `0` 表示审计完整且没有 finding，`1` 表示审计完整但
存在 finding，`2` 表示调用、I/O、超时或容量限制使审计不完整。干净报告不代表文件
系统以后仍可写，也不保证所有 Git 操作一定成功。

## 安全与隐私

- 只读：不执行修复、`prune`、`remove`、`lock`、`unlock` 或主动写探测。
- 离线：无网络请求、遥测或后台服务。
- 显式范围：最多 64 个输入路径；submodule 检查需主动启用，且只检查一层已登记、
  工作目录存在的 submodule。
- 默认脱敏：报告不包含绝对路径、分支名、commit ID、锁定原因、配置值和 Git stderr。
- 有界：元数据文件、Git 输出、命令、协作式整次审计期限、登记 worktree 数和 finding 数均有限制。

JSON 使用 `schema_version: 1`、`kind: "agent_worktree_audit"`、`complete`、
`summary` 和 `findings`。请将未知 finding code 和字段视为向前兼容扩展。

## 文档

- [产品规格](docs/PRODUCT_SPEC.zh-CN.md)
- [安全策略](SECURITY.zh-CN.md)
- [贡献指南](CONTRIBUTING.zh-CN.md)
- [支持](SUPPORT.zh-CN.md)

如果本工具为你节省了时间，可以在 [Ko-fi 支持 Tinkora](https://ko-fi.com/tinkora)。

## 许可证

MIT

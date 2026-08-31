# Agent Worktree Doctor v0.1 产品规格

[English](PRODUCT_SPEC.md)

## 状态与目的

本文定义严格收窄的 v0.1 契约。其中包含可能在 alpha 系列中逐步交付的 finding；
某个具体构建已实现哪些能力，仍以该版本说明和实际可执行行为为准。

本工具只回答一个问题：对于用户明确提供的路径，可观察的 Git worktree 拓扑和元数据
是否存在已知不一致或风险。它不是修复工具、权限判定器或通用仓库健康检查器。

## 公开证据

- Git 在 [git-worktree](https://git-scm.com/docs/git-worktree) 与
  [gitrepository-layout](https://git-scm.com/docs/gitrepository-layout) 中定义了
  `.git` 前向 gitfile、管理目录 `gitdir` 回链、`commondir`，以及稳定的
  `worktree list --porcelain -z` 契约。
- [Codex issue #19786](https://github.com/openai/codex/issues/19786) 记录了
  linked worktree 的私有 gitdir 在 sandbox 中变为只读的问题。
- [Claude Code issue #60131](https://github.com/anthropics/claude-code/issues/60131)
  记录了缺失的 per-worktree submodule gitdir 导致 checkout 失败的问题。
- [Claude Code issue #76144](https://github.com/anthropics/claude-code/issues/76144)
  记录了错误管理回链使仍存活的 worktree 被标记为 prunable 的问题。

这些故障支持本地拓扑诊断，但不构成主动写探测或自动修复的授权。

## 命令契约

```text
agent_worktree_doctor audit <PATH>... [--include-submodules]
  [--check-tracked-status] [--format text|json|sarif]
```

- 必须提供 1 至 64 个路径。
- `text` 是面向人的默认格式，`json` 是稳定结构化输出，`sarif` 面向代码扫描消费者。
- `--include-submodules` 只检查一层已登记且工作目录存在的 submodule，不递归发现仓库。
- `--check-tracked-status` 启用成本可能更高的已跟踪变更检查；它仍然只读且不报告文件名。

退出状态：

| 代码 | 含义 |
| --- | --- |
| `0` | 审计完整且没有 finding。 |
| `1` | 审计完整且产生一个或多个 finding。 |
| `2` | 调用无效，或 I/O、超时、容量边界导致审计不完整。 |

消费者必须检查 `complete`，不得把不完整审计解释为健康。

## Finding

| 代码 | 符号 | 含义 |
| --- | --- | --- |
| AWD001 | `INPUT_PATH_MISSING` | 明确提供的路径不存在。 |
| AWD002 | `NOT_A_GIT_WORKTREE` | 路径无法识别为 Git worktree。 |
| AWD003 | `GIT_PROBE_FAILED` | 有界只读 Git 查询失败。 |
| AWD004 | `MALFORMED_GITFILE` | `.git` 文件无法按预期指针格式解析。 |
| AWD005 | `FORWARD_GITDIR_MISSING` | `.git` 正向指针的目标不存在。 |
| AWD006 | `COMMONDIR_INVALID` | `commondir` 缺失、格式错误或解析到无效位置。 |
| AWD007 | `BACKWARD_GITDIR_MISSING` | worktree 管理目录缺少 `gitdir` 反向指针。 |
| AWD008 | `BIDIRECTIONAL_LINK_MISMATCH` | worktree 正向与反向链接不一致。 |
| AWD009 | `WORKTREE_PRUNABLE` | Git 报告某个已登记 worktree 可被 prune。 |
| AWD010 | `LOCKED_WORKTREE_UNAVAILABLE` | 已锁定的登记 worktree 当前不可用。 |
| AWD011 | `EXPLICIT_CORE_WORKTREE` | 有效且显式的 `core.worktree` 超出 v0.1 审计边界。 |
| AWD012 | `SUBMODULE_GITDIR_MISSING` | 范围内已登记 submodule 指向缺失的 Git 元数据。 |
| AWD013 | `SUBMODULE_SCOPE_MISMATCH` | 范围内 submodule 工作目录与登记元数据不一致。 |
| AWD014 | `TRACKED_CHANGES_PRESENT` | 主动启用的状态检查发现已跟踪变更；不包含文件名。 |
| AWD015 | `AUDIT_INCOMPLETE` | 资源或观察边界使结果无法完整。 |

该表定义标识符和预期类别，并不承诺检测每一种损坏形态。Git 行为、平台路径语义和
不可访问元数据都会限制可观察范围。

## 输出与隐私契约

JSON 文档使用：

```json
{
  "schema_version": 1,
  "kind": "agent_worktree_audit",
  "complete": true,
  "summary": {},
  "findings": []
}
```

`summary` 和 finding 对象内的字段可以兼容扩展。消费者应以数字代码为主要依据，并
容忍未知字段与代码。

默认输出不得暴露绝对路径、分支名、commit ID、锁定原因、配置值或捕获的 Git stderr。
Finding 使用有界、非敏感标签区分同一报告内的输入。文件系统行为和用户提供的标签仍
可能泄露上下文，因此公开报告前仍应检查并脱敏。

## 只读信任边界

允许的观察仅限文件系统元数据读取和不会修改仓库状态的有界 Git 查询。本产品绝不执行
修复、`git worktree prune`、`git worktree remove`、`git worktree lock`、
`git worktree unlock` 或主动写探测；不编辑配置、不创建锁、不扫描整台机器寻找仓库、
不访问网络，也不作为服务运行。

通过 `PATH` 解析出的 `git` 可执行文件属于调用者的信任边界。每一个 `PATH` 条目必须是
绝对路径；如果空条目或相对条目可能从当前工作目录选择可执行文件，审计会在启动 Git 前
fail closed。用户仍有责任提供只包含可信绝对目录的 `PATH`。

任何有效且显式的 `core.worktree` 值均超出 v0.1 审计边界。审计会报告 `AWD011`、将
结果标记为不完整，并在从 Git 报告的 worktree 根目录派生或读取文件之前停止处理该输入。

只读审计无法证明未来可写性、ACL 行为、挂载健康、sandbox 权限或后续可变 Git 命令
一定安全。

## 资源限制

| 资源 | v0.1 限制 |
| --- | --- |
| 输入路径 | 64 |
| 已登记 worktree | 10,000 |
| 单个元数据文件 | 64 KiB |
| 捕获的 Git stdout | 16 MiB |
| 捕获的 Git stderr | 16 MiB |
| 单次 Git 查询 | 5 秒 |
| 协作式整次审计期限 | 60 秒 |
| Finding | 10,000 |

超过限制时必须产生不完整审计并以状态 `2` 退出，不得静默宣称结果干净。

60 秒期限采用协作式执行：工具会在有界 Git 子进程运行期间以及每次观察之间检查期限。
元数据只有在打开前确认是常规文件且未超过大小限制后才会读取，因此 FIFO 会被拒绝而不
会被阻塞读取。对于操作系统文件系统调用（例如访问不可用的网络挂载），工具无法保证严
格的墙钟时间上限。

## 非目标

- 修复或删除 worktree 或元数据。
- 通过实际写入证明路径可写。
- 恢复删除文件或代替备份工具。
- 进程监管或 MCP server 清理。
- 完整递归 submodule 验证、对象完整性（`git fsck`）、远端可用性、认证、hooks、
  构建健康或应用正确性检查。

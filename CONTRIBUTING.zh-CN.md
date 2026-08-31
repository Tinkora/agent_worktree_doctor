# 贡献指南

[English](CONTRIBUTING.md)

扩大产品边界或修改输出契约前，请先创建 issue。保持改动聚焦，添加面向结果的测试，
并使用英文 Conventional Commits。本仓库的代码和代码注释使用英文。

提交 pull request 前请运行：

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

不得添加修复、主动写探测、隐式仓库发现、网络访问、遥测，或在默认报告中包含敏感
路径与 Git 元数据。行为变化时应同步维护中英文文档。

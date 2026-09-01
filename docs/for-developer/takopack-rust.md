# takopack-rust

takopack-rust crate 是 libtakopack 系列中的 Rust 生态系统相关组件，主要负责如下类型的任务：

- 利用本地 Cargo.toml 文件的元数据解析。
- 直接从 crates.io registry 获取已发布包并进行元数据解析。
- 于上述任务相关的辅助函数的开发。

## Status

目前 takopack-rust 正在进行重构（~~重写~~），目标是使其更适合作为一个独立的 crate 被其他项目（或 TakoPack CLI 本身）调用。

### 重构目标

1. 为核心功能设计符合 Rust 风格的入口。
2. 实现各功能模块的清晰分层，以可读性和可维护性作为目标。
3. 所依赖的 crates 尽量最少和最小化。

## Modules

### 'cargo'

使用 Cargo 构建系统和 crates.io 的项目相关的解析，同时也是重构后的入口，新开发的功能应以此为基础。

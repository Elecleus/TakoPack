# TakoPack

TakoPack 是一个用于将各种语言生态系统的软件（目前支持 Rust/Cargo 和 Python/PyPI）打包为 Linux 发行版 RPM spec 文件的工具。

## 功能特性

- **单包打包**: 为单个 crate 生成 RPM spec 文件
- **本地打包**: 直接从本地 Cargo.toml 生成 spec 文件
- **Python 打包**: 为单个 Python 包生成 RPM spec 文件
- **Registry 同步**: 从 ruyispec 同步 crate 到本地 registry，支持增量更新和并发下载
- **依赖检查**: 验证 crate 能否从本地 registry 解析依赖
- **BuildRequires 生成**: 自动生成 RPM 的 BuildRequires 声明

## 安装

```bash
cargo install --path crates/takopack
```

或从源码构建：

```bash
cargo build --release
```

## 配置文件

TakoPack 使用 `takopack.toml` 配置文件来设置默认路径。

### 配置文件位置

按以下顺序查找，找到第一个即停止：

1. `./takopack.toml`（当前工作目录）
2. `~/.config/takopack/takopack.toml`（Linux）
3. `~/Library/Application Support/takopack/takopack.toml`（macOS）
4. `C:\Users\{user}\AppData\Roaming\takopack\takopack.toml`（Windows）

### 配置项

```toml
[ruyispec]
# ruyispec 仓库路径，registry-sync 命令需要
local_path = "/path/to/openruyi-repo"

[registry]
# 本地 Cargo registry 目录
# 可选，默认为 $XDG_DATA_HOME/takopack/cargo-registry
local_path = "/path/to/cargo-registry"
```

### 相对路径

`local_path` 支持相对路径，相对于配置文件所在目录：

```toml
# 配置文件: /home/user/project/takopack.toml
[ruyispec]
local_path = "../openruyi-repo"  # 实际路径: /home/user/openruyi-repo
```

### 默认 registry 路径

如果未配置 `[registry].local_path`，使用以下默认路径：

- Linux: `~/.local/share/takopack/cargo-registry`
- macOS: `~/Library/Application Support/takopack/cargo-registry`
- Windows: `C:\Users\{user}\AppData\Roaming\takopack\cargo-registry`

### 配置示例

```toml
[ruyispec]
local_path = "/root/git/openruyi-repo"

[registry]
local_path = "/root/git/takopack-cargo-registry"
```

## 命令别名

为了方便使用，主要命令提供了以下简短别名：

- `package` → `pkg`
- `localpkg` → `local`

## 输出格式

所有生成的 spec 文件遵循 RPM spec 格式，包含：

- 正确的 `crate()` provides/requires 声明
- 来自 Cargo 依赖的版本约束
- 正确处理特性（feature）依赖
- 自动提取许可证和元数据

## 环境变量

- `RUST_LOG`: 设置日志级别（例如：`RUST_LOG=debug takopack cargo pkg serde`）

## Future Support

Takopack is designed to support multiple language ecosystems:

- ✅ Rust/Cargo (currently implemented)
- 🚧 Perl/CPAN (planned)
- ✅ Python/PyPI (currently implemented)
- 🚧 Go modules (planned)

## 许可证

本项目采用 MIT OR Apache-2.0 许可证。

## 贡献

欢迎贡献！请随时提交 issue 和 pull request。

## 使用方法

TakoPack 的 Rust/Cargo 操作都在 `cargo` 子命令下：

当前保留的 Cargo 子命令包括 `pkg`、`localpkg`、`registry-sync`、`resolve-check` 和 `buildreqs`。批量文件处理、vendor 递归入口、Cargo.toml 递归解析入口以及手动 crates.io 缓存刷新入口已不再提供。

### 1. pkg - 打包单个 Crate

从 crates.io 下载并为单个 crate 生成 RPM spec 文件。

```bash
# 打包指定版本
takopack cargo pkg <CRATE_NAME> <VERSION>

# 打包最新版本
takopack cargo pkg <CRATE_NAME>

# 指定输出目录
takopack cargo pkg <CRATE_NAME> <VERSION> --directory output_dir

# 临时输出 TakoPack 内置 SPDX 头
takopack cargo pkg <CRATE_NAME> <VERSION> --with-spdx

# 示例
takopack cargo pkg serde 1.0.210
takopack cargo pkg tokio
```

**输出**:

- 默认（无 `--directory`）：创建 `rust-{crate}-{compat_version}/` 目录
- 指定 `--directory output_dir`：创建 `output_dir/rust-{crate}-{compat_version}/` 目录

目录内容：

- `rust-{crate}-{compat_version}.spec` - RPM spec 文件
- `Cargo.toml` - 归一化的 Cargo.toml 文件

版本兼容性规则（compat_version）：

- `1.x.y` → `1`（主版本兼容）
- `0.x.y` → `0.x`（次版本兼容，x > 0）
- `0.0.x` → `0.0.x`（补丁版本兼容）
- 预发布版本使用完整版本号（如 `0.26.0-beta.1`）

**特点**:

- 自动下载指定版本的 crate
- 生成符合 RPM 规范的 spec 文件
- 自动提取许可证和元数据信息
- 处理特性（feature）依赖

### 2. localpkg - 本地打包

从本地目录或 Cargo.toml 文件直接生成 spec 文件，无需下载。适用于开发中的项目或自定义的 crate。

```bash
# 从目录打包（目录需包含 Cargo.toml）
takopack cargo localpkg <PATH>

# 从 Cargo.toml 文件打包
takopack cargo localpkg path/to/Cargo.toml

# 指定输出目录
takopack cargo localpkg <PATH> -o output_dir

# 临时输出 TakoPack 内置 SPDX 头
takopack cargo localpkg <PATH> --with-spdx

# 示例
takopack cargo localpkg ./my-project
takopack cargo localpkg ./Cargo.toml -o specs/
```

**输出**:

- 默认（无 `-o`）：在当前目录创建 `rust-{crate}-{compat_version}/` 子目录，包含 spec 和 Cargo.toml
- 指定 `-o output_dir`：创建 `output_dir/rust-{crate}-{compat_version}/` 子目录，包含 spec 和 Cargo.toml

**特点**:

- 无需上传到 crates.io 即可生成 spec
- 适合本地开发和测试
- 支持路径为目录或直接指向 Cargo.toml 文件
- 自动处理本地依赖关系

### 3. registry-sync - 同步 Registry

从 ruyispec 仓库同步 Rust crate 到本地 Cargo registry 目录。用于构建本地离线 registry，供 `resolve-check` 和 `buildreqs` 使用。

```bash
# 同步（使用配置文件中的路径）
takopack cargo registry-sync

# 试运行，只显示计划，不实际修改
takopack cargo registry-sync --dry-run

# 指定并发数（默认 8）
takopack cargo registry-sync -j 4
```

**参数**:

- `--dry-run`: 只打印同步计划，不修改文件
- `-j, --jobs N`: 并发下载/解压的线程数，默认 8

**输出示例**:

```
Registry sync
  ruyispec: /path/to/openruyi-repo
  registry: /path/to/cargo-registry
  jobs: 8

Summary:
  add=5
  update=2
  remove=0
  skip=100
  warnings=0
  sync_errors=0
```

**特点**:

- 增量同步：通过 SHA-256 hash 对比，只更新变化的 crate
- 并发下载：多线程并行下载，提高效率
- 安全机制：marker 文件防止误操作非托管目录
- 原子更新：先写临时目录，再 rename 替换

### 4. resolve-check - 依赖解析检查

验证单个 Cargo crate 能否使用本地 registry 完成依赖解析。用于检查本地 registry 的完整性。

```bash
# 检查当前目录的 crate
takopack cargo resolve-check .

# 检查指定路径
takopack cargo resolve-check ./path/to/crate

# 指定 registry 目录（覆盖配置文件）
takopack cargo resolve-check . --registry /path/to/registry
```

注：请使用此命令来检查仓库使用 `crate` 构建的应用包的依赖是否满足。

**输出示例**:

```
Resolve check
  manifest: /path/to/crate/Cargo.toml
  registry: /path/to/cargo-registry

Result: ok
```

**返回值**:

- `0`: 解析成功，所有依赖都在本地 registry 中
- `1`: 解析失败，有依赖缺失

### 5. buildreqs - 生成 BuildRequires

从 Cargo 依赖解析结果自动生成 RPM 的 BuildRequires 声明。

```bash
# 为当前目录的 crate 生成 BuildRequires
takopack cargo buildreqs .

# 为指定路径生成
takopack cargo buildreqs ./path/to/crate

# 指定 registry 目录
takopack cargo buildreqs . --registry /path/to/registry
```

**输出示例**:

```
BuildRequires:  crate(serde-1) >= 1.0.210
BuildRequires:  crate(tokio-1) >= 1.40.0
BuildRequires:  crate(anyhow-1) >= 1.0.86
```

**特点**:

- 自动过滤：只输出 registry 来源的依赖，排除本地路径依赖
- 版本兼容：使用 compat version 规则（如 `1.x.y` → `1`）
- 去重排序：自动去重并按字母顺序排列

注：目前输出的构建依赖比较冗长，可以考虑后续结合 `feature` 进行缩减。

## 使用示例

### 1: 打包单个 Crate

```bash
# 打包 serde 1.0.210
takopack cargo pkg serde 1.0.210
```

输出结构：

```
rust-serde-1/
├── rust-serde-1.spec
└── Cargo.toml
```

### 2: 本地项目打包

```bash
# 为当前项目生成 spec
takopack cargo localpkg ./Cargo.toml

# 为另一个项目生成 spec
takopack cargo localpkg ../other-project -o specs/
```

## 使用方法

TakoPack 的 Python/PyPI 操作在 `py` 子命令下：

```bash
# 打包最新版本
takopack py package <NAME>

# 打包指定版本
takopack py package <NAME> <VERSION>

# 指定输出目录
takopack py package <NAME> -o output_dir
```

Python 功能已内置：使用 `takopack py package <NAME> [VERSION] [-o output_dir]` 即可生成 Python 包对应的 RPM spec（默认输出到 `python-{srcname}/python-{srcname}.spec`）。

# 开发指南

工具链：Rust 1.97（见 `rust-toolchain.toml`）、Node.js 24+、`frontend/package.json` 指定的 pnpm；本地 QA 另需 Bun。
依赖固定在 `backend/Cargo.lock` 与 `frontend/pnpm-lock.yaml`，不需要同级宿主源码。
SDK 版本以 `backend/Cargo.toml` 中 `gateway-plugin-sdk` 的 `rev` 为准，宿主版本要求以 `plugin.json` 的 `engines` 为准。

## 发布（无需本地构建）

`.github/workflows/release.yml` 在稳定版 `vMAJOR.MINOR.PATCH` 标签推送时运行；预发布标签不发布为 Latest。
先保持 `plugin.json`、`backend/Cargo.toml`、`frontend/package.json` 版本一致，并更新 `release/notes.md` 的版本标题，再提交、推送代码及对应标签。
也可手动运行 Release 工作流，只需填写已存在的 `tag`；所选运行 ref 必须与该标签指向同一提交，否则工作流拒绝发布。

工作流将在全新的 Ubuntu 24.04 环境执行：

1. 核对远端标签、当前提交、版本与 SDK 锁定提交
2. 前端 Lint、测试、类型检查及构建，后端 Rustfmt、Clippy 与完整测试
3. 使用固定的 `cargo-zigbuild==0.23.4`、`ziglang==0.14.1` 构建 Linux x86_64 GNU（glibc 2.28 基线）release 二进制，不复用本地产物
4. 用 Bun 1.4.2 启动该 release 二进制，验证协议 2、56 个账号的隐私投影、HTTP/WS 观测、设置保存与 CAS 409
5. 从 `backend/Cargo.toml` 动态读取 SDK 提交，安装同提交的官方 `cpr-plugin` 并打包；检查 ELF 架构、GLIBC 版本引用、清单/协议合同、每个包内文件与摘要
6. 创建 Draft Release，仅上传 `.tar.gz` 与 `.sha256`；下载全部附件并核对文件名、摘要及本次构建内容，再发布为非预发布的 Latest

已有同名 Release（含 Draft）时拒绝覆盖。上传或回读失败时不会主动公开草稿，应先检查 Actions 日志和草稿内容，再由维护者决定清理后重跑；不要覆盖已公开版本。
这些是工作流的发布门禁，不代表云端运行已经通过。Linux 正式宿主安装与真实上游流量仍需单独验收，协议替身不连接 PostgreSQL 或 Redis。

## 可选本地检查与构建

检查命令参照 CI（`.github/workflows/ci.yml`）：

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend lint
pnpm --dir frontend test
pnpm --dir frontend build
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo clippy --manifest-path backend/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path backend/Cargo.toml --locked
```

日常开发可构建本机二进制；这些命令不是发布前置要求：

```sh
cargo build --manifest-path backend/Cargo.toml --locked
```

手动复现交付构建时，需先安装与工作流一致的 `cargo-zigbuild` 和 Zig，并通过 `CARGO_ZIGBUILD_ZIG_PATH` 指向 Zig。
单纯使用原生 `cargo build --target x86_64-unknown-linux-gnu` 会链接构建环境的 glibc，不能自动保证 2.28 基线：

```sh
rustup target add x86_64-unknown-linux-gnu --toolchain 1.97.0
cargo zigbuild --manifest-path backend/Cargo.toml --release --locked --target x86_64-unknown-linux-gnu.2.28
```

## 本地 QA

`scripts/qa-host.mjs` 启动本机编译的插件二进制，经协议握手、管理路由和观测调用生成页面数据；
宿主账号与 CAS 存储是测试替身，不连接 PostgreSQL、Redis 或真实上游，也不包含在插件包中。

```sh
cargo build --manifest-path backend/Cargo.toml --locked
pnpm --dir frontend build
bun scripts/qa-host.mjs
```

打开输出的 `http://127.0.0.1:4178`（可用 `QA_PORT` 修改）。仅调页面样式时可运行 `pnpm --dir frontend dev`，
打开 `/preview.html` 夹具页面，参数见 [DESIGN.md](../DESIGN.md)。
两者都不等同于在 Linux 正式宿主中安装并以真实流量验证。

## 可选手动打包

打包器必须与 SDK 使用同一上游提交，从 `backend/Cargo.toml` 读取：

```sh
SDK_REV=$(sed -nE 's/^gateway-plugin-sdk = .*rev = "([0-9a-f]+)".*/\1/p' backend/Cargo.toml)
cargo install --locked --git https://github.com/zyycn/codex-proxy-rs.git \
  --rev "$SDK_REV" codex-proxy-plugin-cli --root .tools
.tools/bin/cpr-plugin package --manifest plugin.json \
  --binary backend/target/x86_64-unknown-linux-gnu/release/codex-proxy-state-plugin \
  --target x86_64-unknown-linux-gnu --resource-map web=frontend/dist --output-dir dist
```

生成的 `.tar.gz` 与 `.sha256` 通过摘要绑定二进制和静态资源。
在 Linux 上可用 `python3 scripts/verify-package.py --tag v0.2.0 --directory dist --binary <本次构建的二进制路径>` 复现包校验（需 Python 3.11+、`readelf`）。
进程冒烟命令为 `QA_PLUGIN_BINARY=<本次构建的二进制路径> bun scripts/qa-smoke.mjs`。

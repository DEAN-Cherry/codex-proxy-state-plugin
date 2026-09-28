# 开发指南

工具链：Rust 1.97（见 `rust-toolchain.toml`）、Node.js 24+、`frontend/package.json` 指定的 pnpm；本地 QA 另需 Bun。
依赖固定在 `backend/Cargo.lock` 与 `frontend/pnpm-lock.yaml`，不需要同级宿主源码。
SDK 版本以 `backend/Cargo.toml` 中 `gateway-plugin-sdk` 的 `rev` 为准，宿主版本要求以 `plugin.json` 的 `engines` 为准。

## 检查与构建

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

CI 只做本机 release 构建；打包用的二进制需显式指定安装包目标：

```sh
cargo build --manifest-path backend/Cargo.toml --release --locked --target x86_64-unknown-linux-gnu
```

在非 Linux 主机交叉构建时可用 `cargo-zigbuild`，按其文档通过 `CARGO_ZIGBUILD_ZIG_PATH` 指向 Zig：

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

## 打包

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

## 发布

- 版本标签指向已通过本地检查和 `main` CI 的提交，源码、`plugin.json` 版本与附件保持一致
- 将 `.tar.gz` 和 `.sha256` 上传到对应 GitHub Release，不覆盖已发布的同版本内容
- 稳定版不标记 Draft 或 Pre-release，宿主留空标签时才能查到
- 发布成功不等于已在目标宿主生效，Linux 宿主安装与真实上游流量需在目标环境验收

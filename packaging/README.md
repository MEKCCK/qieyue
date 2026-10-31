# 打包

两个平台的入口都在这个目录；图标统一来自 `assets/branding/`。桌面端是 Tauri v2：
前端静态资源已经嵌入 `qieyue` 二进制，打包只需要把二进制和图标放进平台格式。

## macOS

```sh
./packaging/macos/bundle.sh
open build/macos/QieYue.app
```

生成 `build/macos/QieYue.app`，包含 `Info.plist`、可执行文件和 `qieyue.icns`。
`.icns` 由 `scripts/generate-branding.sh` 从 SVG 生成，并按苹果图标网格留了透明边距，
所以 Dock 里不会显得比其他应用大。脚本只做 ad-hoc 签名，正式分发需要用自己的证书签名并公证。

直接用 `target/debug/qieyue` 启动时，macOS 没有 bundle 图标可用，Dock 会显示通用可执行文件图标；
要在 Dock 看到项目图标，请通过 `.app` 启动。

也可以用 Tauri 官方打包器（会读取 `desktop/tauri.conf.json` 和 `desktop/icons/`）：

```sh
cargo install tauri-cli --version "^2" --locked   # 只需一次
cargo tauri build --config desktop/tauri.conf.json
```

## Linux

```sh
./packaging/linux/install.sh
```

安装到 `~/.local/bin/qieyue`、`~/.local/share/applications/qieyue.desktop` 和
`~/.local/share/icons/hicolor/512x512/apps/qieyue.png`。系统级安装：

```sh
PREFIX=/usr/local sudo ./packaging/linux/install.sh
```

运行需要系统 WebView：`webkit2gtk-4.1`、GTK3 和 libsoup3（Debian/Ubuntu 安装
`libwebkit2gtk-4.1-0` 等运行时包即可）。也可以用 `cargo tauri build` 生成 `.deb`/AppImage。

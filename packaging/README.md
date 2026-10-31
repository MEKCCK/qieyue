# 打包

两个平台的入口都在这个目录；图标统一来自 `assets/branding/`。

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

## Linux

```sh
./packaging/linux/install.sh
```

安装到 `~/.local/bin/qieyue`、`~/.local/share/applications/qieyue.desktop` 和
`~/.local/share/icons/hicolor/512x512/apps/qieyue.png`。系统级安装：

```sh
PREFIX=/usr/local sudo ./packaging/linux/install.sh
```

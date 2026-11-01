# 构建与维护

这里只放构建、运行和维护相关的说明；项目是什么、有哪些功能见 [README](../README.md)。

## 依赖

支持 **Linux** 与 **macOS**。界面是 Tauri v2 + 系统 WebView，前端为原生 HTML/CSS/JS，
不需要 Node / npm / Vite，也不需要打包 Chromium。

- `rustc` / `cargo`（stable 即可）
- Tauri v2 的 WebView 运行时：
  - macOS：Xcode Command Line Tools（使用系统 WKWebView）
  - Linux：`webkit2gtk-4.1`、`libgtk-3-dev`、`libsoup-3.0-dev`、`librsvg2-dev`
    （Debian/Ubuntu 可装 `libwebkit2gtk-4.1-dev build-essential librsvg2-dev`）
- `mpv`（播放短剧与听书，`--demuxer-lavf-o=decryption_key` 需要 ffmpeg 的 CENC 支持）
  - Linux：`sudo apt install mpv`（或发行版对应命令）
  - macOS：`brew install mpv`
  - 程序会先探测常见的安装路径（含 Homebrew），也可以设置 `QIEYUE_MPV` 指定可执行文件
- 中文字体：使用系统字体（macOS 苹方、Linux Noto Sans CJK 等）

## 构建

整个仓库是一个 Cargo workspace，三个成员：`rust/`（上游核心 `fqapi_core`，原样副本）、
`desktop/`（Tauri v2 外壳 + `ui/` 前端）和 `fqcore/`（无界面调试守护进程）。

```sh
./scripts/cargo.sh build --release     # 或加 -p qieyue 只编界面
```

`scripts/cargo.sh` 是 cargo 的直接替换（用法一样，只是不写开头的 `cargo`）：
机器上装了 `mbx`（mise 的 mr_boxington 编译缓存）时，编译走 mbx 的共享缓存，
重编和跨代码检出复用会快很多；没装 `mbx`（CI、新机器）时自动退回普通 cargo。
它只影响本仓库的构建入口，不安装全局 shim。

并行方面不需要额外配置：Cargo 默认就用全部逻辑 CPU 并行编译各个 crate，
release 的 `codegen-units` 也保持默认的 16（crate 内代码生成同样并行）。
`rust/Cargo.toml` 里的 `[profile.*]` 在 workspace 下会被忽略（cargo 会给出 warning），
不要把它挪到根 `Cargo.toml`，否则 `codegen-units = 1` 会让代码生成退化为单线程。

产物在 `target/release/`：`qieyue`（Tauri 桌面端）、`smoke`（无界面自检）和 `fqcore`（REST 调试守护进程）。

安装到用户目录（Linux / macOS 通用，确保 `~/.local/bin` 在 `PATH` 里）：

```sh
install -m 755 target/release/qieyue ~/.local/bin/qieyue
```

## 运行

```sh
qieyue
# 开发时也可以直接跑 Cargo
./scripts/cargo.sh run -p qieyue
```

首次运行会生成配置、书架与解密缓存，位置随平台而定：

- Linux：`$XDG_DATA_HOME/qieyue/`，未设置时是 `~/.local/share/qieyue/`
- macOS：`~/Library/Application Support/qieyue/`
- `QIEYUE_DATA_DIR` 可以覆盖以上全部（测试或便携安装用）

目录内容：

- `config.json`：首次运行把内置模板写出来
- `shelf.json`：书架与阅读进度
- `run/src/`：短剧下载的分集与漫画解密出的图片

## 自检

界面之外的所有数据链路都有无界面验证，改完先跑它：

```sh
./scripts/cargo.sh run --release --bin smoke -- 斗破
```

它会依次验证：搜索与分页、详情与目录、正文解析、书架增删、短剧集数、视频/音频地址与 CENC 密钥、
首页各频道的卡片数与翻页、分类、排行榜、漫画解密。

## 界面调试

前端是 `desktop/ui/` 里的静态文件，由 Tauri 编译时嵌入二进制。

- WebView 里的 JS 异常会通过 `debug_log` 命令转到终端，输出前缀是 `[webview]`；
  常见启动日志依次是 `boot`、`backend ready`、`home loaded: …`。
- debug 构建可以打开 WebView 的开发者工具检查 DOM/网络；release 构建不开。
- 后端数据链路仍用下面的无界面自检验证，不依赖窗口。

## 品牌资源

`assets/branding/` 里是可提交的成品；源文件是两张 SVG：

- `icon.svg`：应用图标（Linux 图标、窗口图标、应用内标识）
- `logo.svg`：横版标识（README / 网站）

已提交的 PNG 和 macOS `.icns` 用脚本重新生成；脚本同时把 Tauri 需要的图标同步到
`desktop/icons/`，把应用内标识同步到 `desktop/ui/assets/logo.png`：

```sh
./scripts/generate-branding.sh
```

需要 `rsvg-convert`；macOS 的 `.icns` 还需要 `iconutil`/`sips` 和 ImageMagick。
macOS 图标会按苹果的图标网格留出透明边距（832/1024），避免在 Dock 里显得过大；
同时有一圈淡描边，深色背景下也能看清。

## 打包

两种方式等价，仓库自带的脚本不依赖 Tauri CLI：

1. 用仓库脚本把 Tauri 二进制包成平台产物：
   - macOS（生成 `build/macos/QieYue.app`，带 Dock 图标）：
     ```sh
     ./packaging/macos/bundle.sh
     open build/macos/QieYue.app
     ```
   - Linux（安装到 `~/.local`，需要系统已装 WebKitGTK 运行时）：
     ```sh
     ./packaging/linux/install.sh
     ```
     系统级安装用 `PREFIX=/usr/local sudo ./packaging/linux/install.sh`。
2. 用 Tauri 官方打包器：
   ```sh
   cargo install tauri-cli --version "^2" --locked   # 只需一次
   ./scripts/cargo.sh build --release -p qieyue
   cargo tauri build --config desktop/tauri.conf.json
   ```
   生成 `.app`/`.dmg`（macOS）或 `.deb`/AppImage（Linux），图标与版本来自
   `desktop/tauri.conf.json` 和 `desktop/icons/`。

细节见 [packaging/README](../packaging/README.md)。

## 同步上游

上游是 [ch6vip/fqapp](https://github.com/ch6vip/fqapp)。`rust/` 是它 `rust/` 目录的
**原样副本**，不作为本仓库的修改目标：界面代码只放在 `desktop/`，调试守护进程只放在
`fqcore/`，两边都通过路径依赖使用核心。保持这一点，上游随时可以整体替换进来。

```sh
./scripts/sync-backend.sh            # 同步到上游默认分支
./scripts/sync-backend.sh master     # 指定分支
./scripts/sync-backend.sh v1.2.3     # 指定 tag
./scripts/sync-backend.sh --check    # 只检查 rust/ 是否与上游一致
```

脚本用 sparse + blobless clone 只拉取上游的 `rust/`，然后用 rsync 整体替换（含删除），
最后再比对一次，确保结果与上游逐字节一致。同步后跑一次
`./scripts/cargo.sh check --workspace`：上游依赖或接口变化会在这里暴露。

不要在 `rust/` 里提交本地修改。确实需要补丁时，先和上游对齐，或把适配层放在
`desktop/`，否则下一次同步会直接覆盖掉。

## 已知限制

- 番茄账号导入（书架 / 历史）未实现：需要接上游的网页登录与 cookie 导入流程
- 首页「排行榜」区块只是预览，完整榜单在排行榜页

# qieyue desktop

Linux / macOS 的 Tauri v2 桌面端：Rust 宿主 + 系统 WebView（macOS WKWebView、
Linux WebKitGTK）+ 原生 HTML/CSS/JS。业务逻辑直接用仓库里的 `fqapi_core` 后端
（协议、解密、缓存都在核心里），播放短剧与听书时调用系统 `mpv`。

前端是纯静态文件，经 Tauri 编译时嵌入二进制；不需要 Node / npm / Vite。

## 源码布局

    src/main.rs          Tauri 入口与命令注册
    src/commands.rs      IPC 命令（薄封装，调用 qieyue 适配层）
    src/lib.rs           后端适配层（模块声明 + 对外导出）
    src/{backend,models,parse,paths,search,book,series,shelf,...}.rs
                         请求、数据模型、解析、路径、搜索、详情、书架、音频、评论、漫画、榜单
    src/mpv.rs           mpv 探测与启动
    src/bin/smoke.rs     无界面数据链路自检
    ui/index.html        壳层结构
    ui/styles.css        深色主题
    ui/app.js            标签页、页面栈、IPC 调用与渲染
    tauri.conf.json      窗口、权限、图标、打包配置
    capabilities/        Tauri v2 capability
    icons/               Tauri 使用的图标（由 scripts/generate-branding.sh 同步）

## 界面结构

左侧四个根标签：**首页 / 书架 / 排行榜 / 分类**。每个标签有自己的页面栈；
从书架打开一本书、切到首页再回来，仍停在原来的页面。页面记录它打开的内容 id，
别的标签换了书之后，旧页面不会显示错内容。

## 性能与加载

- 每条 IPC 命令由 Tauri 的 async runtime 并发执行；前端 `call` 层最多 8 个请求同时在飞，
  不会串行排队。
- 每个请求带单调 id；同域出现更新的请求时，旧回复直接丢弃。
- 首页各区块的 `series-feed` 在 Rust 侧用 `join_all` 并发拉取。
- 分页只追加 DOM，不整页重绘；没有新行时立即停止，避免无限翻页。
- 自动翻页只在用户真实滚动后触发（600ms 冷却），不滚动就不会轮询。
- 图片使用原生 `loading="lazy"`；首页区块默认渲染 60 张，可手动展开。
- 书架状态在内存里判断，渲染时不读盘；标签根数据只在首次进入时加载。

## 运行

```sh
./scripts/cargo.sh run -p qieyue
# 或
cargo run -p qieyue
```

无界面自检（搜索 → 详情 → 首章 → 短剧/漫画/音频等数据链路）：

```sh
./scripts/cargo.sh run --release --bin smoke -- 斗破
```

数据目录：Linux 为 `~/.local/share/qieyue`（或 `$XDG_DATA_HOME/qieyue`），
macOS 为 `~/Library/Application Support/qieyue`，可用 `QIEYUE_DATA_DIR` 覆盖。
漫画图片通过 Tauri 的 asset protocol 从该目录读取。

## 平台与打包

只支持 Linux 与 macOS；其他平台会在编译期报错（见 `lib.rs`）。中文字体由系统 WebView
使用系统字体栈，mpv 可用 `QIEYUE_MPV` 指定。

macOS 用 `./packaging/macos/bundle.sh` 生成带 Dock 图标的 `.app`；Linux 用
`./packaging/linux/install.sh` 安装桌面项与图标。也可以用 Tauri CLI：
`cargo install tauri-cli --version "^2" --locked && cargo tauri build`。
详见 [packaging/README](../packaging/README.md)。

## 调试

debug 构建里 WebView 的 JS 异常会通过 `debug_log` 命令打到终端，前缀为 `[webview]`；
正常启动日志依次是 `boot`、`backend ready`、`home loaded: …`。debug 构建可以打开
WebView 开发者工具检查 DOM 与网络。

# qieyue desktop

Linux / macOS 的纯 Rust 界面，用 egui/eframe 写，直接链接仓库里的 `fqapi_core`
后端（协议、解密、缓存都在核心里），不需要 Flutter、Dart 或任何 WebView。
播放短剧与听书时调用系统 `mpv`。

## 源码布局

    src/main.rs        eframe 启动入口、窗口图标
    src/app.rs         应用状态、标签页导航栈、命令/消息泵、视图分发
    src/theme.rs       深色主题与强调色
    src/worker.rs      后台 async worker（所有后端请求）
    src/platform.rs    平台差异：中文字体、mpv 探测/启动、品牌资源
    src/views/nav.rs   左侧导航：标识、首页/书架/排行/分类、版本
    src/views/chrome.rs
                       顶栏：返回、标题、搜索、状态
    src/views/         其余界面模块（首页/搜索/书架/详情/阅读/漫画/分类/榜单）
    src/lib.rs         后端适配层（模块声明 + 对外导出）
    src/{backend,models,parse,paths,...}.rs
                       请求、数据模型、解析、路径、搜索、详情、书架、音频、评论、漫画、榜单

界面模块只画 UI 并发送 `Cmd`，不直接发网络请求；`lib.rs` 里不出现 egui 类型。

## 界面结构

左侧四个根标签：**首页 / 书架 / 排行榜 / 分类**。每个标签有自己的页面栈，页面是
首页、搜索结果、详情、阅读器、漫画、短剧等；切换标签会恢复该标签上次的位置，
再次点击当前标签回到根页面。页面记录它打开的内容 id，别的标签换了书之后，
旧页面不会再显示错内容。

## 性能

- 请求并发：`worker.rs` 为每条命令启动独立任务（最多 8 个同时在飞），每条回复带请求 id，
  界面只接受同域最新的 id，慢请求不会覆盖新界面。
- 首页各区块的 `series-feed` 用 `join_all` 并发拉取。
- 列表按行虚拟化：搜索结果、榜单、书架、首页每个区块只渲染视口内的行。
- 视图直接借用 `App` 里的数据，不再每帧克隆整个首页/目录/章节列表。
- 渲染时不读 `shelf.json`；书架在内存里判断。漫画每帧只解码 2 页。
- 标签根数据只在首次进入时加载，失败后点击标签重试，不会每帧重发。

## 运行

```sh
./scripts/cargo.sh run --release -p qieyue
# 或
cargo run --release -p qieyue
```

无界面自检（搜索 → 详情 → 首章 → 短剧/漫画/音频等数据链路）：

```sh
./scripts/cargo.sh run --release --bin smoke -- 斗破
```

数据目录：Linux 为 `~/.local/share/qieyue`（或 `$XDG_DATA_HOME/qieyue`），
macOS 为 `~/Library/Application Support/qieyue`，可用 `QIEYUE_DATA_DIR` 覆盖。

## 平台与打包

只支持 Linux 与 macOS；其他平台会在编译期直接报错。中文字体与 mpv 的查找逻辑
都在 `src/platform.rs`，可用 `QIEYUE_CJK_FONT`、`QIEYUE_MPV` 覆盖。

macOS 用 `./packaging/macos/bundle.sh` 生成带 Dock 图标的 `.app`；Linux 用
`./packaging/linux/install.sh` 安装桌面项与图标。详见 [packaging/README](../packaging/README.md)。

## 调试图标 / 截图

`QIEYUE_SCREENSHOT=/tmp/ui.png`（可选 `QIEYUE_SCREENSHOT_DELAY_MS`，默认 2500）
会在首屏加载稳定后读取窗口帧缓冲写成 PNG，然后退出；不需要屏幕录制权限，适合 UI 评审和 CI。

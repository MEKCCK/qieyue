# qieyue desktop

Linux / macOS 的纯 Rust 界面，用 egui/eframe 写，直接链接仓库里的 `fqapi_core`
后端（协议、解密、缓存都在核心里），不需要 Flutter、Dart 或任何额外工具链。
播放短剧与听书时调用系统 `mpv`。

## 源码布局

    src/main.rs        eframe 启动入口
    src/app.rs         应用状态、命令/消息泵、视图分发
    src/worker.rs      后台 async worker（所有后端请求）
    src/platform.rs    平台差异：中文字体、mpv 探测/启动
    src/views/         每个界面一个模块（首页/搜索/书架/详情/阅读/漫画/分类/榜单）
    src/lib.rs         后端适配层（模块声明 + 对外导出）
    src/{backend,models,parse,paths,...}.rs
                       请求、数据模型、解析、路径、搜索、详情、书架、音频、评论、漫画、榜单

界面模块只画 UI 并发送 `Cmd`，不直接发网络请求；`lib.rs` 里不出现 egui 类型。

请求是并发的：`worker.rs` 为每条命令启动独立任务（最多 8 个同时在飞），每条回复带请求 id，
界面只接受同域最新的 id，慢请求返回时不会覆盖新界面。首页各区块的 `series-feed` 也并发拉取。
分页、评论等守卫按域判断，互不阻塞；书架状态在内存里判断，渲染时不再读盘。

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

## 平台

只支持 Linux 与 macOS；其他平台会在编译期直接报错。中文字体与 mpv 的查找逻辑
都在 `src/platform.rs`，可用 `QIEYUE_CJK_FONT`、`QIEYUE_MPV` 覆盖。

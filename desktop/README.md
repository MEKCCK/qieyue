# fqapp desktop

一个纯 Rust 的 Linux 前端，用 egui/eframe 写界面，直接链接仓库里的 `fqapi_core` 后端
（它负责协议、解密、缓存），所以不需要 Flutter、Dart 或任何额外工具链。

已有的功能：

- 搜索（书名 / 作者 / 短剧），结果显示封面、书名、作者、简介，短剧条目带标签
- 书籍详情：简介 + 完整目录（列表可滚动、点击跳章）
- 正文阅读：段落排版、滚动、上一章 / 下一章
- 书架：读过的书自动记进 `shelf.json`，左侧面板可一键回到上次的章节
- 短剧：剧集列表，点击后解析出视频文件与 CENC 密钥，交给系统 mpv 播放

已知空缺：听书朗读、漫画看图、清晰度切换。

## 运行

```sh
cargo run --release --bin fqapp-desktop
```

无界面自检（搜索 → 详情 → 首章，打印在终端）：

```sh
cargo run --release --bin smoke -- 斗破
```

数据目录是 `~/.local/share/fqapp-desktop`，首次运行会从 `assets/config/config.json`
复制一份配置。

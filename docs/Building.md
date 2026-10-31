# 构建与维护

这里只放构建、运行和维护相关的说明；项目是什么、有哪些功能见 [README](../README.md)。

## 依赖

只用到 Rust 工具链和系统里的 mpv，不需要 Flutter / Dart / CMake / ninja：

- `rustc` / `cargo`（stable 即可）
- `mpv`（播放短剧与听书，`--demuxer-lavf-o=decryption_key` 需要 ffmpeg 的 CENC 支持）
- 中文字体（默认找 `JetBrains Maple Mono`，找不到就用 `Noto Sans CJK`）

## 构建

整个仓库是一个 Cargo workspace，两个成员：`rust/`（上游核心 `fqapi_core`）和 `desktop/`（界面）。

```sh
cargo build --release          # 或 -p qieyue 只编界面
```

产物在 `target/release/`：`qieyue`（界面）和 `smoke`（无界面自检）。

安装到系统：

```sh
sudo install -Dm755 target/release/qieyue /usr/local/bin/qieyue
```

## 运行

```sh
qieyue
```

首次运行会在 `~/.local/share/qieyue/` 下生成配置与书架：

- `config.json`：从 `assets/config/config.json` 复制而来
- `shelf.json`：书架与阅读进度
- `run/src/`：短剧下载的分集与漫画解密出的图片

## 自检

界面之外的所有数据链路都有无界面验证，改完先跑它：

```sh
cargo run --release --bin smoke -- 斗破
```

它会依次验证：搜索与分页、详情与目录、正文解析、书架增删、短剧集数、视频/音频地址与 CENC 密钥、
首页各频道的卡片数与翻页、分类、排行榜、漫画解密。

## 演示模式

方便截图与排查，用环境变量驱动界面走到指定状态：

- `FQAPP_DEMO_QUERY` + `FQAPP_DEMO_STAGE`（1 搜索结果 / 2 详情 / 3 正文）
- `FQAPP_DEMO_CHANNEL=8:5:12` 切到指定频道（tab_type:tab_index:client_template）
- `FQAPP_DEMO_MANGA=<book_id>` 直接打开某本漫画第一话
- `FQAPP_DEMO_SERIES=<series_id>` 直接打开某个短剧
- `FQAPP_DEMO_VIEW=library|rank|category` 直接进对应页面

## 同步上游

上游是 [ch6vip/fqapp](https://github.com/ch6vip/fqapp)，本项目只新增了 `desktop/`，`rust/` 保持原样。
跟进上游时：

```sh
git remote add upstream https://github.com/ch6vip/fqapp.git   # 只需一次
git fetch upstream main
git merge upstream/main
```

冲突通常只会出现在 `rust/` 里的接口实现；`desktop/` 按上游 `lib/services/api_client.dart`
的调用方式跟着改即可（路径与参数一一对应）。

## 已知限制

- 番茄账号导入（书架 / 历史）未实现：上游用内嵌 WebView 登录拿 cookie，纯 Rust 版需要另想办法
- 短剧清晰度切换未接（接口已返回 `variants` 列表）
- 首页「排行榜」区块只是预览，完整榜单在排行榜页

# 构建与维护

这里只放构建、运行和维护相关的说明；项目是什么、有哪些功能见 [README](../README.md)。

## 依赖

支持 **Linux** 与 **macOS**，只用到 Rust 工具链和系统里的 mpv，不需要 Flutter / Dart / CMake / ninja：

- `rustc` / `cargo`（stable 即可）
- `mpv`（播放短剧与听书，`--demuxer-lavf-o=decryption_key` 需要 ffmpeg 的 CENC 支持）
  - Linux：`sudo apt install mpv`（或发行版对应命令）
  - macOS：`brew install mpv`
  - 程序会先探测常见的安装路径（含 Homebrew），也可以设置 `QIEYUE_MPV` 指定可执行文件
- 中文字体：
  - Linux：`JetBrains Maple Mono`、`Noto Sans CJK` 等
  - macOS：`PingFang`、`STHeiti`、`Noto Sans CJK` 等
  - 找不到时会让中文显示为方框，可用 `QIEYUE_CJK_FONT` 指向任意 TTF/TTC 字体

## 构建

整个仓库是一个 Cargo workspace，三个成员：`rust/`（上游核心 `fqapi_core`，原样副本）、
`desktop/`（界面）和 `fqcore/`（无界面调试守护进程）。

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

产物在 `target/release/`：`qieyue`（界面）、`smoke`（无界面自检）和 `fqcore`（REST 调试守护进程）。

安装到用户目录（Linux / macOS 通用，确保 `~/.local/bin` 在 `PATH` 里）：

```sh
install -m 755 target/release/qieyue ~/.local/bin/qieyue
```

## 运行

```sh
qieyue
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

## 演示模式

方便截图与排查，用环境变量驱动界面走到指定状态：

- `FQAPP_DEMO_QUERY` + `FQAPP_DEMO_STAGE`（1 搜索结果 / 2 详情 / 3 正文）
- `FQAPP_DEMO_CHANNEL=8:5:12` 切到指定频道（tab_type:tab_index:client_template）
- `FQAPP_DEMO_MANGA=<book_id>` 直接打开某本漫画第一话
- `FQAPP_DEMO_SERIES=<series_id>` 直接打开某个短剧
- `FQAPP_DEMO_VIEW=library|rank|category` 直接进对应页面

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

- 番茄账号导入（书架 / 历史）未实现：上游用内嵌 WebView 登录拿 cookie，纯 Rust 版需要另想办法
- 短剧清晰度切换未接（接口已返回 `variants` 列表）
- 首页「排行榜」区块只是预览，完整榜单在排行榜页

# 茄阅 · QieYue

<p align="center">
  <img src="assets/branding/icon-256.png" width="120" alt="茄阅 QieYue">
</p>

一个 **Tauri v2 + 纯 Rust 后端** 的 Linux / macOS 桌面客户端：看小说、看漫画、追短剧、听书。

界面用 **Tauri v2** 写（系统 WebView + 原生 HTML/CSS/JS），业务逻辑直接复用上游项目的
Rust 核心 `fqapi_core`（协议、签名、解密、缓存都在核心里）。不依赖 Flutter、Dart、CMake，
也不需要 Node/npm 或打包 Chromium：macOS 用系统 WKWebView，Linux 用系统 WebKitGTK。

## 它是什么

茄阅是 [fqapp](https://github.com/ch6vip/fqapp)（Android / Windows 客户端，Flutter UI + 进程内 Rust 核心）
的 **Linux / macOS 桌面移植版**。上游把"内容"和"界面"分得很干净：Rust 核心对外是一组
`/api/v1/...` 接口，Flutter 只是这些接口的消费者。所以移植没有重写业务，只重写了前端：

    desktop/ (Tauri v2 + WebView)  ──IPC──▶  fqapi_core (上游 Rust 核心)  ──▶  上游服务

只支持 Linux 与 macOS；Windows 用上游的 Flutter 客户端。视频与音频交给系统里的 `mpv`
（含 CENC 解密密钥），不内嵌播放器。

界面是左侧标签式导航：**首页 / 书架 / 排行榜 / 分类**。每个标签有自己的页面栈，
从书架打开一本书、切到首页再回来，仍停在原来的阅读页；详情、阅读器、漫画和短剧都在
标签内以页面形式打开。图片使用原生懒加载，首页区块默认渲染 60 张并可展开；切标签不会重新加载已有数据。

## 功能

- **首页**：11 个频道（推荐 / 小说 / 经典 / 知识 / 听书 / 看剧 / 视频 / 短篇 / 漫画 / 新书 / 商城），
  区块滚动到底自动加载下一页
- **搜索**：书名 / 作者 / 短剧，分页加载，短剧条目带标记
- **分类**：频道（出版 / 男生 / 女生 / 有声书）+ 维度标签（主题 / 角色 / 情节，共 194 个），点标签直接搜
- **排行榜**：6 个榜单 + 16 个分类，带序号与封面，每页 30 条自动续载
- **书籍详情**：封面、简介、完整目录、评论（昵称 / 日期 / 赞与回复数）
- **阅读器**：段落排版、滚动、上一章 / 下一章
- **漫画阅读器**：按话解密成图片，竖向连续翻页、居中显示、上一话 / 下一话
- **短剧 / 视频**：剧集列表 → 解析视频地址与 CENC 密钥 → `mpv` 播放；
  多档清晰度（1080P / 720P / 540P / 480P / 360P）先选档再起播
- **听书**：音色选择、底部播放条（进度条可拖动、暂停/继续、上一章/下一章、自动续播下一章），
  关闭窗口时一并结束 mpv
- **听书**：章节音频 → `mpv` 朗读，音色按"默认 → 真人讲书 → 智能朗读"依次尝试
- **书架**：书籍与短剧都能加入，记录读到第几章 / 第几集，点击直接续读

## 仓库结构

    rust/      上游 Rust 核心（fqapi_core）：协议、签名、解密、缓存
    desktop/   本项目的界面（Tauri v2 外壳 + ui/ 前端），通过 IPC 调核心
    fqcore/    无界面的核心守护进程，用于单独调试 REST 接口
    assets/    核心用的配置模板与过滤器脚本
    scripts/   构建、编译缓存与上游同步脚本
    docs/      构建与维护说明

Flutter、Android、Windows 相关代码全部不在本仓库内。

## 与上游的关系

- 业务逻辑、接口、解密全部来自上游 `rust/` 里的 `fqapi_core`，本仓库没有改动它的协议实现
- `rust/` 是上游 `rust/` 目录的**原样副本**，任何时候都可以整体替换；
  用 `./scripts/sync-backend.sh` 同步，`--check` 可确认当前是否一致
- 界面部分 `desktop/` 是新增的，上游只有 Flutter（Android / Windows）前端
- 上游 README 保留在 `README.upstream.md`，构建与维护说明见 `docs/Building.md`

## 许可

GPL-3.0。本项目是 [fqapp](https://github.com/ch6vip/fqapp) 的衍生作品，
原项目 Copyright (C) 2026 ch6vip，以 GPL-3.0 发布；本仓库的修改同样以 GPL-3.0 发布，
完整协议见 [LICENSE](LICENSE)。

## 免责声明

本项目只做客户端界面，不提供、不托管任何内容，所有内容均来自上游服务。
仅供学习与个人使用，请在遵守当地法律法规与内容服务条款的前提下使用。

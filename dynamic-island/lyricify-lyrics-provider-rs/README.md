# Lyricify Lyrics Provider
## 声明

逻辑(C#源代码)源于[Lyricify-Lyrics-Helper](https://github.com/WXRIW/Lyricify-Lyrics-Helper)

以下下内容**由ai代笔**

网络歌词获取库，提供音乐平台 API 调用、智能搜索匹配和 SMTC 一站式歌词获取管线。

## 功能

- **Providers** — 网易云、QQ音乐、酷狗、汽水音乐的 API 客户端
- **Searchers** — 多轮智能搜索 + 自动评分排序，返回最佳匹配
- **SMTC 歌词管线** — 传入歌曲信息，自动检测运行中的播放器进程，用自家源获取歌词

## 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
lyricify-lyrics-provider = { path = "../lyricify-lyrics-provider-rs" }
tokio = { version = "1", features = ["full"] }
```

> 本库已内置歌词解析、数据模型和辅助工具模块，无需额外声明 `helper` 依赖。

## 快速上手

### SMTC 一站式获取歌词

```rust
use lyricify_lyrics_provider::smtc_lyrics;

#[tokio::main]
async fn main() {
    match smtc_lyrics::get_lyrics(
        "晴天",              // 歌曲名（必填）
        Some("周杰伦"),      // 歌手名（可选）
        Some("叶惠美"),      // 专辑名（可选）
        None,                // 时长毫秒（可选）
    ).await {
        Ok((player, lyrics)) => {
            println!("通过 {} 获取到 {} 行歌词",
                player.display_name(), lyrics.lines.len());
        }
        Err(e) => eprintln!("获取失败: {}", e),
    }
}
```

**内部流程**：检测进程 → 按首字母排序 (K→N→Q→S) → 取第一个 → 用自家源搜索+获取歌词 → 返回 `LyricsData`

### 指定播放器源

```rust
use lyricify_lyrics_provider::smtc_lyrics::{self, MusicPlayer};

let lyrics = smtc_lyrics::get_lyrics_with_player(
    &MusicPlayer::Netease,
    "晴天", Some("周杰伦"), None, None,
).await?;
```

### 进程检测

```rust
use lyricify_lyrics_provider::smtc_lyrics;

let players = smtc_lyrics::get_running_players();
if let Some(first) = smtc_lyrics::get_first_running_player() {
    println!("将使用: {}", first.display_name());
}
```

### 直接调用平台 API

```rust
use lyricify_lyrics_provider::providers::netease::NeteaseApi;

let api = NeteaseApi::new();
let result = api.search("晴天 周杰伦", 1).await?;
let lyric = api.get_lyric("186016").await?;
```

### 智能搜索

```rust
use lyricify_lyrics_provider::searchers::{ISearcher, netease::NeteaseSearcher};

let searcher = NeteaseSearcher::new();
let best = searcher.search_for_result(&track_metadata).await?;
```

### 访问解析/模型/工具模块

```rust
use lyricify_lyrics_provider::parsers;
use lyricify_lyrics_provider::models;
use lyricify_lyrics_provider::helpers;
```

## 支持的播放器

| 播放器 | 枚举值 | 进程名 | 歌词源 |
|--------|--------|--------|--------|
| 酷狗音乐 | `MusicPlayer::Kugou` | `KGMusic.exe` | 酷狗 API |
| 网易云音乐 | `MusicPlayer::Netease` | `cloudmusic.exe` | 网易云 API（优先 YRC 逐字，回退 LRC） |
| QQ音乐 | `MusicPlayer::QQMusic` | `QQMusic.exe` | QQ音乐 API |
| 汽水音乐 | `MusicPlayer::SodaMusic` | `SodaMusic.exe` | 汽水音乐 API |

## 模块结构

```
src/
├── lib.rs              # 入口，导出 models/helpers/parsers/providers/searchers/smtc_lyrics
├── models/             # 歌词数据模型
├── helpers/            # 通用辅助工具
├── parsers/            # LRC / YRC 解析
├── smtc_lyrics.rs      # SMTC 一站式歌词获取管线
├── providers/          # 平台 API 客户端
│   ├── base_api.rs     # HTTP 基础封装
│   ├── proxy.rs        # 代理配置
│   ├── netease.rs      # 网易云音乐 API
│   ├── qqmusic.rs      # QQ音乐 API
│   ├── kugou.rs        # 酷狗音乐 API
│   └── soda_music.rs   # 汽水音乐 API
└── searchers/          # 智能搜索
    ├── helpers/        # 搜索辅助逻辑
    │   └── compare.rs  # 匹配算法
    ├── netease.rs
    ├── qqmusic.rs
    ├── kugou.rs
    └── soda_music.rs
```

## 代理设置

```rust
use lyricify_lyrics_provider::providers::proxy;
use lyricify_lyrics_provider::providers::netease::NeteaseApi;

let client = proxy::create_proxy_client("127.0.0.1", 7890, None, None)?;
let api = NeteaseApi::with_client(client);
```

## 许可证

Apache-2.0

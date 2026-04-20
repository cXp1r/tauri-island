# Lyricify Lyrics Provider
## 声明

仓库自用,并提供给自己参与开发的仓库用
parsers实现µs级别解析(<1ms无需预热)

Api接口抓取歌词源于[Lyricify-Lyrics-Helper](https://github.com/WXRIW/Lyricify-Lyrics-Helper)

## 功能

- **Providers** — 网易云、QQ音乐、酷狗、汽水音乐的 API 客户端
- **Searchers** — 弱智评分机制 + 神人匹配字符串，返回最佳匹配
- **SMTC 歌词管线** — 传入歌曲信息，自动检测运行中的播放器进程，用自家源获取歌词

## 安装

在 `Cargo.toml` 中添加：

```toml
[dependencies]
lyricify-lyrics-provider = { path = "../lyricify-lyrics-provider-rs" }
tokio = { version = "1", features = ["full"] }
```


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


### 访问解析/模型/工具模块

```rust
use lyricify_lyrics_provider::parsers;
use lyricify_lyrics_provider::models;
use lyricify_lyrics_provider::helpers;
```

## 支持的播放器

| 播放器 | 枚举值 | 进程名 | 歌词源 |
|--------|--------|--------|--------|
| 酷狗音乐 | `MusicPlayer::Kugou` | `KuGou.exe` | 酷狗 API |
| 网易云音乐 | `MusicPlayer::Netease` | `cloudmusic.exe` | 网易云 API（优先 YRC 逐字，回退 LRC） |
| QQ音乐 | `MusicPlayer::QQMusic` | `QQMusic.exe` | QQ音乐 API |
| 汽水音乐 | `MusicPlayer::SodaMusic` | `SodaMusic.exe` | 汽水音乐 API |

## 模块结构

```text
src/
├── lib.rs
├── smtc_lyrics.rs
├── models/
│   ├── mod.rs
│   ├── additional_file_info.rs
│   ├── file_info.rs
│   ├── line_info.rs
│   ├── lyrics_data.rs
│   ├── lyrics_types.rs
│   ├── sync_types.rs
│   └── track_metadata.rs
├── parsers/
│   ├── mod.rs
│   ├── attributes_helper.rs
│   ├── kugou.rs
│   ├── lrc.rs
│   ├── netease.rs
│   ├── qqmusic.rs
│   ├── soda_music.rs
│   └── decrypt/
│       ├── mod.rs
│       ├── krc.rs
│       └── qrc.rs
├── providers/
│   ├── mod.rs
│   ├── base_api.rs
│   ├── kugou.rs
│   ├── netease.rs
│   ├── proxy.rs
│   ├── qqmusic.rs
│   └── soda_music.rs
└── searchers/
    ├── mod.rs
    ├── kugou.rs
    ├── netease.rs
    ├── qqmusic.rs
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

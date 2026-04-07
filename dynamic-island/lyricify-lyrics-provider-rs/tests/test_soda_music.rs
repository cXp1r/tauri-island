#[tokio::test]
async fn test_soda_music_search_and_lyric() {
    use lyricify_lyrics_provider::providers::soda_music::SodaMusicApi;

    let api = SodaMusicApi::new();

    // 1) 搜索原唱（非翻唱/钢琴版）
    let keyword = "又活了一天 庄东茹";
    println!("[soda-test] search keyword='{}'", keyword);
    let search_result = api.search(keyword).await;
    match &search_result {
        Ok(Some(r)) => {
            let groups = r.result_groups.as_ref().map(|g| g.len()).unwrap_or(0);
            println!("[soda-test] search ok, groups={}", groups);
            if let Some(groups) = &r.result_groups {
                for group in groups {
                    if let Some(items) = &group.data {
                        for (i, item) in items.iter().enumerate().take(5) {
                            if let Some(entity) = &item.entity {
                                if let Some(track) = &entity.track {
                                    let id = track.id.as_deref().unwrap_or("?");
                                    let name = track.name.as_deref().unwrap_or("?");
                                    let dur = track.duration.unwrap_or(0);
                                    let artists: Vec<&str> = track.artists.as_ref()
                                        .map(|a| a.iter().filter_map(|x| x.name.as_deref()).collect())
                                        .unwrap_or_default();
                                    println!("[soda-test] search [{}] id={} name='{}' artists={:?} duration={}ms",
                                        i, id, name, artists, dur);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(None) => println!("[soda-test] search returned None"),
        Err(e) => println!("[soda-test] search error: {}", e),
    }

    // 2) 取第一条结果的 id 去拿详情
    let first_id = search_result.ok().flatten().and_then(|r| {
        r.result_groups?.into_iter().find_map(|g| {
            g.data?.into_iter().find_map(|item| {
                item.entity?.track?.id
            })
        })
    });

    let id = match first_id {
        Some(id) => {
            println!("[soda-test] using id={}", id);
            id
        }
        None => {
            println!("[soda-test] no track id found, skip get_detail");
            return;
        }
    };

    // 3) get_detail (先打原始 JSON)
    println!("[soda-test] get_detail id='{}'", id);
    {
        use lyricify_lyrics_provider::providers::base_api::BaseApi;
        let base = BaseApi::new(None, None);
        let raw_url = format!(
            "https://api.qishui.com/luna/pc/track_v2?track_id={}&media_type=track&queue_type=",
            urlencoding::encode(&id)
        );
        match base.get_async(&raw_url).await {
            Ok(raw) => println!("[soda-test] raw response (len={}):\n{}", raw.len(), &raw[..raw.len().min(4000)]),
            Err(e) => println!("[soda-test] raw request error: {}", e),
        }
    }
    // 直接测 serde 反序列化，打出具体错误
    {
        use lyricify_lyrics_provider::providers::base_api::BaseApi;
        let base = BaseApi::new(None, None);
        let raw_url = format!(
            "https://api.qishui.com/luna/pc/track_v2?track_id={}&media_type=track&queue_type=",
            urlencoding::encode(&id)
        );
        if let Ok(raw) = base.get_async(&raw_url).await {
            let parse_result = serde_json::from_str::<lyricify_lyrics_provider::providers::soda_music::TrackDetailResult>(&raw);
            match parse_result {
                Ok(v) => println!("[soda-test] serde ok, lyric_content_len={}",
                    v.lyric.as_ref().and_then(|l| l.content.as_ref()).map(|c| c.len()).unwrap_or(0)),
                Err(e) => println!("[soda-test] serde error: {}", e),
            }
        }
    }
    match api.get_detail(&id).await {
        Ok(Some(detail)) => {
            let track_name = detail.track.as_ref().and_then(|t| t.name.as_deref()).unwrap_or("?");
            println!("[soda-test] get_detail ok, track='{}'", track_name);
            match &detail.lyric {
                Some(lyric) => {
                    let content_len = lyric.content.as_ref().map(|c| c.len()).unwrap_or(0);
                    let lyric_type = lyric.lyric_type.as_deref().unwrap_or("?");
                    let lang = lyric.lang.as_deref().unwrap_or("?");
                    let preview: String = lyric.content.as_deref().unwrap_or("").chars().take(200).collect();
                    println!("[soda-test] lyric type={} lang={} length={}", lyric_type, lang, content_len);
                    println!("[soda-test] lyric preview:\n{}", preview);
                }
                None => println!("[soda-test] no lyric in detail"),
            }
        }
        Ok(None) => println!("[soda-test] get_detail returned None (parse failed or empty)"),
        Err(e) => println!("[soda-test] get_detail error: {}", e),
    }

    // 4) 验证 KRC 解析（直接调 smtc_lyrics 全流程）
    {
        use lyricify_lyrics_provider::smtc_lyrics;
        match smtc_lyrics::get_lyrics_with_player(
            &smtc_lyrics::MusicPlayer::SodaMusic,
            "又活了一天（DJ黄周豪版）",
            Some("庄东茹（豆芽鱼）"),
            None,
            None,
        ).await {
            Ok(data) => {
                println!("[soda-test] smtc_lyrics ok, lines={}", data.lines.len());
                for (i, line) in data.lines.iter().take(5).enumerate() {
                    println!("[soda-test] line[{}] time={:?} text='{}'",
                        i, line.start_time(), line.text());
                }
            }
            Err(e) => println!("[soda-test] smtc_lyrics error: {}", e),
        }
    }

    // 5) 探测独立歌词接口 /luna/pc/lyric
    println!("[soda-test] trying /luna/pc/lyric endpoint...");
    {
        use lyricify_lyrics_provider::providers::base_api::BaseApi;
        let base = BaseApi::new(None, None);
        let lyric_url = format!(
            "https://api.qishui.com/luna/pc/lyric?track_id={}&media_type=track",
            urlencoding::encode(&id)
        );
        match base.get_async(&lyric_url).await {
            Ok(raw) => println!("[soda-test] /luna/pc/lyric response:\n{}", &raw[..raw.len().min(1000)]),
            Err(e) => println!("[soda-test] /luna/pc/lyric error: {}", e),
        }
    }
}

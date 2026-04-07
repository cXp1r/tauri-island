use crate::helpers::string_helper::between;
use crate::models::{
    AdditionalFileInfo, LyricsData, TrackMetadata,
};

/// 是否是 Attribute 信息行
pub fn is_attribute_line(line: &str) -> bool {
    let line = line.trim();
    line.starts_with('[') && line.ends_with(']') && line.contains(':')
}

/// 获取 Attribute 信息
fn get_attribute(line: &str) -> (String, String) {
    let line = line.trim();
    let key = between(line, "[", ":").unwrap_or_default();
    let colon_pos = line.find(':').unwrap_or(0);
    let value = &line[colon_pos + 1..line.len() - 1];
    (key, value.to_string())
}

/// 将 Attributes 信息解析到 LyricsData 中 (从行列表)
/// 返回 Offset 值
pub fn parse_general_attributes_to_lyrics_data(
    data: &mut LyricsData,
    lines: &mut Vec<String>,
) -> Option<i32> {
    let mut offset: Option<i32> = None;

    if data.track_metadata.is_none() {
        data.track_metadata = Some(TrackMetadata::default());
    }

    while !lines.is_empty() {
        let i = 0;
        if is_attribute_line(&lines[i]) {
            let (key, value) = get_attribute(&lines[i]);
            let meta = data.track_metadata.as_mut().unwrap();

            match key.as_str() {
                "ar" => meta.artist = Some(value.clone()),
                "al" => meta.album = Some(value.clone()),
                "ti" => meta.title = Some(value.clone()),
                "length" => {
                    if let Ok(v) = value.parse::<i32>() {
                        meta.duration_ms = Some(v);
                    }
                }
                "offset" => {
                    if let Ok(v) = value.parse::<i32>() {
                        offset = Some(v);
                    }
                }
                _ => {}
            }

            if key == "hash" {
                if let Some(AdditionalFileInfo::Krc(ref mut krc_info)) =
                    data.file.as_mut().and_then(|f| f.additional_info.as_mut())
                {
                    krc_info.hash = Some(value);
                }
            } else if let Some(file_info) = data.file.as_mut() {
                match file_info.additional_info {
                    Some(AdditionalFileInfo::General(ref mut info)) => {
                        info.attributes.push((key, value));
                    }
                    Some(AdditionalFileInfo::Krc(ref mut info)) => {
                        info.attributes.push((key, value));
                    }
                    _ => {}
                }
            }

            lines.remove(i);
        } else {
            break;
        }
    }

    offset
}

/// 将 Attributes 信息从原始字符串解析到 LyricsData 中
/// 返回 (offset, 解析结束后的字符位置)
pub fn parse_general_attributes_from_string(
    data: &mut LyricsData,
    input: &str,
) -> (Option<i32>, usize) {
    let mut offset: Option<i32> = None;

    if data.track_metadata.is_none() {
        data.track_metadata = Some(TrackMetadata::default());
    }

    let mut index = 0;
    let chars: Vec<char> = input.chars().collect();

    while index < chars.len() {
        if chars[index] == '[' {
            let end_index = input[index..].find('\n').map(|p| index + p);
            if let Some(end_idx) = end_index {
                let info_line = &input[index..end_idx];
                if is_attribute_line(info_line) {
                    let (key, value) = get_attribute(info_line);
                    let meta = data.track_metadata.as_mut().unwrap();

                    match key.as_str() {
                        "ar" => meta.artist = Some(value.clone()),
                        "al" => meta.album = Some(value.clone()),
                        "ti" => meta.title = Some(value.clone()),
                        "length" => {
                            if let Ok(v) = value.parse::<i32>() {
                                meta.duration_ms = Some(v);
                            }
                        }
                        "offset" => {
                            if let Ok(v) = value.parse::<i32>() {
                                offset = Some(v);
                            }
                        }
                        _ => {}
                    }

                    if let Some(file_info) = data.file.as_mut() {
                        if let Some(AdditionalFileInfo::General(ref mut info)) =
                            file_info.additional_info
                        {
                            info.attributes.push((key, value));
                        }
                    }

                    index = end_idx;
                } else {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
        index += 1;
    }

    (offset, index)
}

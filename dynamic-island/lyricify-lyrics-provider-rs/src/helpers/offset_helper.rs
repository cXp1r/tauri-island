use crate::models::{LineInfo, Syllable};

/// 为歌词行列表添加偏移量
pub fn add_offset_to_lines(lines: &mut Vec<LineInfo>, offset: i32) {
    for line in lines.iter_mut() {
        add_offset_to_line(line, offset);
    }
}

/// 为单个歌词行添加偏移量
pub fn add_offset_to_line(line: &mut LineInfo, offset: i32) {
    match line {
        LineInfo::Basic(l) => {
            l.start_time = l.start_time.map(|t| t - offset);
            l.end_time = l.end_time.map(|t| t - offset);
        }
        LineInfo::Syllable(l) => {
            add_offset_to_syllables(&mut l.syllables, offset);
        }
        LineInfo::Full(l) => {
            l.start_time = l.start_time.map(|t| t - offset);
            l.end_time = l.end_time.map(|t| t - offset);
        }
        LineInfo::FullSyllable(l) => {
            add_offset_to_syllables(&mut l.syllables, offset);
        }
    }
}

fn add_offset_to_syllables(syllables: &mut Vec<Syllable>, offset: i32) {
    for syllable in syllables.iter_mut() {
        match syllable {
            Syllable::Basic(s) => {
                s.start_time = s.start_time.map(|t| t - offset);
                s.end_time = s.end_time.map(|t| t - offset);
            }
            Syllable::Full(fs) => {
                fs.start_time = fs.start_time.map(|t| t - offset);
                fs.end_time = fs.end_time.map(|t| t - offset);
                for sub in fs.sub_items.iter_mut() {
                    sub.start_time = sub.start_time.map(|t| t - offset);
                    sub.end_time = sub.end_time.map(|t| t - offset);
                }
            }
        }
    }
}

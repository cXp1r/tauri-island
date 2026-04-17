
use crate::parsers::{IParsers, LRC_LINE_TIMESTAMP};
use crate::models::LineInfo;
use regex::Captures;
pub struct LrcParsers {}
impl IParsers for LrcParsers{
    fn parse_line(&self, caps: Captures<'_>) -> Result<(u32, u32, String), String> {
        let t1 = caps
                        .get(1)
                        .ok_or("Sync Parser: Missing start_time".to_string())?
                        .as_str()
                        .parse::<u32>()
                        .map_err(|_| "Sync Parser: Can't parse start_time".to_string())?;

        let t2 = caps.
                        get(2)
                        .ok_or("Sync Parser: Missing duration".to_string())?
                        .as_str()
                        .parse::<u32>()
                        .map_err(|_| "Sync Parser: Can't parse duration".to_string())?;
        let t3 = caps
                            .get(3)
                            .ok_or("Sync Parser: Missing lyrics")?
                            .as_str()
                            .parse::<u32>()
                            .map_err(|_| "Sync Parser: Can't parse duration".to_string())?;
        let text = caps
                            .get(4)
                            .ok_or("Sync Parser: Missing lyrics")?
                            .as_str()
                            .to_string();
        Ok((t1*60000+t2*1000+t3*10, 0, text))
    }
    fn parse(&self, lyrics: String) -> Result<Vec<LineInfo>, String> {
        /*use std::time::Instant;
        let start = Instant::now();*/
        
        
        let mut lineinfo: Vec<LineInfo> = Vec::new();
        for caps in LRC_LINE_TIMESTAMP.captures_iter(&lyrics) {
            let (s, d, text) = self.parse_line(caps)?;
            
            lineinfo.push(LineInfo {
                start_time: s,
                duration: d as u16,
                text: text,
                syllables: vec![],
            });
        }

        /*let elapsed = start.elapsed();
        println!("解析歌词耗时耗时: {:?}", elapsed);*/


        Ok(lineinfo)
    }
}

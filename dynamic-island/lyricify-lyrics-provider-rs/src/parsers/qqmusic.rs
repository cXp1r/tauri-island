use crate::parsers::{IParsers, decrypt::qrc::*};
use crate::models::LineInfo;
pub struct QQMusicParsers {}
impl QQMusicParsers {
    fn decrypt(&self, lyrics: &str) -> Result<String, String> {
        qrc_decrypt(lyrics)
    }
    pub fn decrypt_and_parse(&self, lyrics: String) -> Result<Vec<LineInfo>, String>  {
        let lyrics = self.decrypt(&lyrics)?;
        //println!("{}",lyrics);
        self.parse(lyrics)
    }
}
impl IParsers for QQMusicParsers{
    
}

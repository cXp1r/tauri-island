use crate::parsers::{IParsers, decrypt::krc::*};
use crate::models::*;
pub struct KugouParser;
impl KugouParser {
    fn decrypt(&self, lyrics: &str) -> Result<String, String> {
        krc_decrypt(lyrics)
    }
    pub fn decrypt_and_parse(&self, lyrics: String) -> Result<Vec<LineInfo>, String>  {
        let lyrics = self.decrypt(&lyrics)?;
        //println!("{}",lyrics);
        self.parse(lyrics)
    }
}
impl IParsers for KugouParser {
    #[allow(unused_variables)]
    fn get_offset_time(&self, t1: u32, t2: u32) -> Result<u16, String> {
        u16::try_from(t2)
            .map_err(|_| format!("Parsers: offset overflow({})",t1))
    }  
}
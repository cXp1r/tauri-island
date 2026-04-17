#[derive(Debug, Clone)]
pub enum LyricsTypes {
    LRC,//路边
    QRC,//感谢大哥的解密算法
    YRC,//路边
    KRC,//感谢大哥的解密算法
    Unknown,//路边
}

impl Default for LyricsTypes {
    fn default() -> Self {
        LyricsTypes::Unknown
    }
}
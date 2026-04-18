#[derive(Debug, Clone, Default)]
pub enum LyricsTypes {
    LRC,//路边
    QRC,//感谢大哥的解密算法
    YRC,//路边
    KRC,//感谢大哥的解密算法
    #[default]
    Unknown,//路边
}

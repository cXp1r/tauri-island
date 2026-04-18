#[derive(Debug, Clone, Default)]
pub struct TextInfo {
    pub start_time: u16,//offset当前行st
    pub duration: u16,//同上
    pub text: String,
}
#[derive(Debug, Clone, Default)]
pub struct LineInfo {
    pub start_time: u32,
    pub duration: u16,//65s够你吃一壶了
    pub text: String,
    pub syllables: Vec<TextInfo>,
}

impl LineInfo {
    pub fn is_empty(&self) -> bool {
        self.start_time == 0 || self.syllables.is_empty()
    }
}
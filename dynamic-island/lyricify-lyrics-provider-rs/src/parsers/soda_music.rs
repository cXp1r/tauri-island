use crate::parsers::{IParsers, PREFIX_RE};
use regex::{Regex};
pub struct SodaParsers {}
impl IParsers for SodaParsers{
    fn get_syllables_re(&self) -> &Regex {
        &PREFIX_RE
    }
    #[allow(unused_variables)]
    fn get_offset_time(&self, t1: u32, t2: u32) -> Result<u16, String> {
        u16::try_from(t2)
            .map_err(|_| format!("Parsers: offset overflow({})",t1))
    }
}

use crate::parsers::{IParsers, PREFIX_RE};
use regex::{Regex};
pub struct NeteaseParsers {}
impl IParsers for NeteaseParsers{
    fn get_syllables_re(&self) -> &Regex {
        &PREFIX_RE
    }
}

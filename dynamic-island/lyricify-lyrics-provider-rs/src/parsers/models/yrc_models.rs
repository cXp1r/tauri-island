use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreditsInfo {
    #[serde(rename = "t")]
    pub timestamp: Option<i32>,
    #[serde(rename = "c")]
    pub credits: Option<Vec<Credit>>,
}

#[derive(Debug, Deserialize)]
pub struct Credit {
    #[serde(rename = "tx")]
    pub text: Option<String>,
    #[serde(rename = "li")]
    pub image: Option<String>,
    #[serde(rename = "or")]
    pub orpheus: Option<String>,
}

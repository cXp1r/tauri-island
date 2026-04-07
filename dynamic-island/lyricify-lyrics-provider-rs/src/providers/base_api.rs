use reqwest::{Client, header};
use std::collections::HashMap;

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
pub const COOKIE: &str = "os=pc;osver=Microsoft-Windows-10-Professional-build-19045-64bit;appver=3.0.2.230891;channel=netease;__remember_me=true";

/// Base API client wrapping reqwest::Client
#[derive(Clone)]
pub struct BaseApi {
    pub client: Client,
    pub http_refer: Option<String>,
    pub additional_headers: Option<HashMap<String, String>>,
}

impl BaseApi {
    pub fn new(http_refer: Option<&str>, additional_headers: Option<HashMap<String, String>>) -> Self {
        Self {
            client: Client::new(),
            http_refer: http_refer.map(|s| s.to_string()),
            additional_headers,
        }
    }

    pub fn with_client(client: Client, http_refer: Option<&str>, additional_headers: Option<HashMap<String, String>>) -> Self {
        Self {
            client,
            http_refer: http_refer.map(|s| s.to_string()),
            additional_headers,
        }
    }

    fn build_headers(&self) -> header::HeaderMap {
        let mut headers = header::HeaderMap::new();
        if let Ok(ua) = header::HeaderValue::from_str(USER_AGENT) {
            headers.insert(header::USER_AGENT, ua);
        }
        if let Some(ref refer) = self.http_refer {
            if let Ok(r) = header::HeaderValue::from_str(refer) {
                headers.insert(header::REFERER, r);
            }
        }
        if let Some(ref additional) = self.additional_headers {
            for (key, value) in additional {
                if let (Ok(k), Ok(v)) = (
                    header::HeaderName::from_bytes(key.as_bytes()),
                    header::HeaderValue::from_str(value),
                ) {
                    headers.insert(k, v);
                }
            }
        }
        headers
    }

    pub async fn get_async(&self, url: &str) -> Result<String, reqwest::Error> {
        let resp = self
            .client
            .get(url)
            .headers(self.build_headers())
            .send()
            .await?
            .error_for_status()?;
        resp.text().await
    }

    pub async fn post_form_async(
        &self,
        url: &str,
        params: &HashMap<String, String>,
    ) -> Result<String, reqwest::Error> {
        let resp = self
            .client
            .post(url)
            .headers(self.build_headers())
            .form(params)
            .send()
            .await?
            .error_for_status()?;
        resp.text().await
    }

    pub async fn post_json_async<T: serde::Serialize + ?Sized>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<String, reqwest::Error> {
        let resp = self
            .client
            .post(url)
            .headers(self.build_headers())
            .json(body)
            .send()
            .await?
            .error_for_status()?;
        resp.text().await
    }

    pub async fn post_string_async(
        &self,
        url: &str,
        body: &str,
    ) -> Result<String, reqwest::Error> {
        let resp = self
            .client
            .post(url)
            .headers(self.build_headers())
            .header(header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await?
            .error_for_status()?;
        resp.text().await
    }
}

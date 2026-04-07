use reqwest::{Client, Proxy};

/// 创建带代理的 HTTP Client
pub fn create_proxy_client(host: &str, port: u16, username: Option<&str>, password: Option<&str>) -> Result<Client, reqwest::Error> {
    let proxy_url = format!("http://{}:{}", host, port);
    let mut proxy = Proxy::all(&proxy_url).unwrap();
    if let (Some(user), Some(pass)) = (username, password) {
        proxy = proxy.basic_auth(user, pass);
    }
    Client::builder().proxy(proxy).build()
}

/// 创建不使用代理的 HTTP Client
pub fn create_no_proxy_client() -> Result<Client, reqwest::Error> {
    Client::builder().no_proxy().build()
}

/// 创建默认 HTTP Client
pub fn create_default_client() -> Result<Client, reqwest::Error> {
    Client::builder().build()
}

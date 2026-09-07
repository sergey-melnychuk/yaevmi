use serde::{Serialize, de::DeserializeOwned};

pub struct Http(reqwest::Client);

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

impl Http {
    pub fn new() -> Self {
        // Some public RPC providers (Cloudflare-fronted ones observed in
        // practice) silently block requests with no/blank User-Agent,
        // returning an HTML challenge page instead of JSON -- surfaces as
        // a confusing "error decoding response body", not a clear auth
        // error. A plain, honest UA is enough to avoid that.
        let client = reqwest::Client::builder()
            .user_agent(concat!("yevm/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self(client)
    }

    pub async fn post<Q: Serialize, R: DeserializeOwned>(
        &self,
        url: &str,
        body: &Q,
    ) -> eyre::Result<R> {
        let response = self.0.post(url).json(body).send().await?.json().await?;
        Ok(response)
    }
}

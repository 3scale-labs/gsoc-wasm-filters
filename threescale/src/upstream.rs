use anyhow::anyhow;
use core::convert::TryFrom;
use core::iter::Extend;
use core::time::Duration;
use serde::{Deserialize, Serialize};
use url::Url;

const DEFAULT_TIMEOUT_MS: u64 = 1000_u64;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Upstream {
    pub name: String,
    pub url: Url,
    // timeout in ms
    pub timeout: Duration,
}

impl Upstream {
    #[allow(dead_code)]
    pub fn set_default_timeout(&mut self, timeout: u64) {
        self.timeout = Duration::from_millis(timeout);
    }

    pub const fn default_timeout(&self) -> u128 {
        self.timeout.as_millis()
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn scheme(&self) -> &str {
        self.url.scheme()
    }

    pub fn authority(&self) -> &str {
        self.url.authority()
    }

    pub fn path(&self) -> &str {
        self.url.path()
    }

    pub fn query_string(&self) -> Option<&str> {
        self.url.query()
    }

    #[allow(clippy::too_many_arguments)]
    fn do_call<C: proxy_wasm::traits::Context>(
        ctx: &C,
        name: &str,
        scheme: &str,
        authority: &str,
        path: &str,
        method: &str,
        headers: Vec<(&str, &str)>,
        body: Option<&[u8]>,
        trailers: Option<Vec<(&str, &str)>>,
        timeout: Duration,
    ) -> Result<u32, anyhow::Error> {
        let mut hdrs = vec![
            (":authority", authority),
            (":scheme", scheme),
            (":method", method),
            (":path", path),
        ];

        hdrs.extend(headers);

        let trailers = trailers.unwrap_or_default();
        let body_str = match body {
            Some(bytes) => String::from_utf8_lossy(bytes),
            None => "(nothing)".into(),
        };
        log::debug!(
            "calling out {} (using {} scheme) with headers -> {:?} <- and body -> {:?} <-",
            name,
            scheme,
            hdrs,
            body_str.as_ref()
        );
        ctx.dispatch_http_call(name, hdrs, body, trailers, timeout)
            .map_err(|e| {
                anyhow!(
                    "failed to dispatch HTTP ({}) call to cluster {} with authority {}: {:?}",
                    scheme,
                    name,
                    authority,
                    e
                )
            })
    }

    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn call<C: proxy_wasm::traits::Context>(
        &self,
        ctx: &C,
        path: &str,
        method: &str,
        headers: Vec<(&str, &str)>,
        body: Option<&[u8]>,
        trailers: Option<Vec<(&str, &str)>>,
        timeout_ms: Option<u64>,
    ) -> Result<u32, anyhow::Error> {
        let extra_path = path.trim_start_matches('/');
        let mut path = self.path().to_string();
        path.push_str(extra_path);

        if let Some(qs) = self.query_string() {
            if !path.contains('?') {
                path.push('?');
            }
            path.push_str(qs);
        }

        Self::do_call(
            ctx,
            self.name(),
            self.scheme(),
            self.authority(),
            path.as_str(),
            method,
            headers,
            body,
            trailers,
            timeout_ms.map_or(self.timeout, Duration::from_millis),
        )
    }
}

pub struct Builder {
    url: url::Url,
}

impl Builder {
    pub fn build(mut self, name: &impl ToString, timeout: Option<u64>) -> Upstream {
        let name = name.to_string();

        // any specified path should always be considered a directory in which to further mount paths
        if !self.url.path().ends_with('/') {
            self.url.set_path(format!("{}/", self.url.path()).as_str());
        }

        Upstream {
            name,
            url: self.url,
            timeout: Duration::from_millis(timeout.unwrap_or(DEFAULT_TIMEOUT_MS)),
        }
    }
}

impl TryFrom<url::Url> for Builder {
    type Error = anyhow::Error;

    fn try_from(url: url::Url) -> Result<Self, Self::Error> {
        if !url.has_authority() {
            return Err(anyhow!("url does not contain an authority"));
        }

        Ok(Self { url })
    }
}

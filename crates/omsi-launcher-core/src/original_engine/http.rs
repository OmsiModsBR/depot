//! The one network client of the original-engine mode: HTTPS GET to the official download
//! hosts only, redirects followed by hand (each hop checked against the host list), a size
//! cap, no cookies, no body sent. Used only when the player confirms a download.

use super::install::{host_allowed, Fetch};
use anyhow::{anyhow, bail, Result};
use std::io::Read;
use std::time::{Duration, Instant};

pub struct HttpsFetch;

/// A download that has not finished in this long is dropped (a slow server cannot keep it open).
const TOTAL_TIME: Duration = Duration::from_secs(20 * 60);

/// A URL as shown in a message: without the query or fragment (a signed address carries a
/// secret there).
fn plain(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

impl Fetch for HttpsFetch {
    fn get(
        &self,
        url: &str,
        max: u64,
        progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<Vec<u8>> {
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(30))
            .user_agent("Project Depot")
            .build();
        let mut url = url.to_string();
        let started = Instant::now();
        for _ in 0..6 {
            if !host_allowed(&url) {
                bail!("{} is not an official download address", plain(&url));
            }
            let resp = agent
                .get(&url)
                .set(
                    "Accept",
                    "application/octet-stream, application/vnd.github+json",
                )
                .call()
                .map_err(|e| {
                    anyhow!(
                        "download failed ({}): {}",
                        plain(&url),
                        plain_err(&e.to_string())
                    )
                })?;
            if (300..400).contains(&resp.status()) {
                url = resp
                    .header("location")
                    .ok_or_else(|| anyhow!("redirect without a place"))?
                    .to_string();
                continue;
            }
            let total = resp
                .header("content-length")
                .and_then(|v| v.parse::<u64>().ok());
            if total.map(|t| t > max).unwrap_or(false) {
                bail!("the download is larger than allowed");
            }
            let mut body = Vec::new();
            let mut r = resp.into_reader().take(max + 1);
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = r.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&buf[..n]);
                if body.len() as u64 > max {
                    bail!("the download is larger than allowed");
                }
                if !progress(body.len() as u64, total) {
                    bail!("cancelled");
                }
                if started.elapsed() > TOTAL_TIME {
                    bail!("the download took too long");
                }
            }
            return Ok(body);
        }
        bail!("too many redirects")
    }
}

/// An error text without the query strings of the addresses it mentions.
fn plain_err(text: &str) -> String {
    text.split(' ').map(plain).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_never_reach_a_message() {
        assert_eq!(plain("https://h/x.zip?sig=secret#f"), "https://h/x.zip");
        assert_eq!(
            plain_err("https://h/x?token=abc: status code 403"),
            "https://h/x status code 403"
        );
    }
}

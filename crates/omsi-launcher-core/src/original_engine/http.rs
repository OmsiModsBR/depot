//! The one network client of the original-engine mode: HTTPS GET to the official download
//! hosts only, redirects followed by hand (each hop checked against the host list), a size
//! cap, no cookies, no body sent. Used only when the player confirms a download.

use super::install::{host_allowed, Fetch};
use anyhow::{anyhow, bail, Result};
use std::io::Read;
use std::time::Duration;

pub struct HttpsFetch;

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
        for _ in 0..6 {
            if !host_allowed(&url) {
                bail!("{url} is not an official download address");
            }
            let resp = agent
                .get(&url)
                .set(
                    "Accept",
                    "application/octet-stream, application/vnd.github+json",
                )
                .call()
                .map_err(|e| anyhow!("download failed: {e}"))?;
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
            }
            return Ok(body);
        }
        bail!("too many redirects")
    }
}

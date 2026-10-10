//! Which addresses the radio may connect to.
//!
//! A station address comes from content (a map's `radio.cfg`, a plugin's list) or from the
//! player's own list, and a playlist or redirect can name another one. Followed blindly, a
//! map could make the game send requests to the player's own machine or LAN (a router's
//! admin page, a local service: a blind SSRF). So an address must be
//!
//! * `https` (plain `http` only when the player opts in with `OMSI_RADIO_ALLOW_HTTP=1`),
//! * without `user:password@`,
//! * a host that is not `localhost`/`*.local`/`*.internal`/a one-label name, and not an IP
//!   literal that is loopback, private, link-local, unspecified, multicast, CGNAT or
//!   otherwise not public.
//!
//! [`PublicOnly`] repeats the address check at connect time on what the name resolves to, so
//! a public-looking name that points at a private address (DNS rebinding, a decimal
//! `2130706433`) is refused too, on every redirect and playlist hop.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

/// Whether plain `http` stations are allowed (`OMSI_RADIO_ALLOW_HTTP=1`).
pub fn http_allowed() -> bool {
    omsi_cfg::env::var("OMSI_RADIO_ALLOW_HTTP").is_ok_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
}

/// Whether `ip` is an address on the public internet.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => is_public_v4(a),
        IpAddr::V6(a) => {
            if let Some(v4) = a.to_ipv4_mapped() {
                return is_public_v4(v4);
            }
            let s = a.segments();
            // ::/96 (IPv4-compatible), loopback, unspecified
            if s[..6] == [0; 6] {
                return false;
            }
            !(a.is_loopback()
                || a.is_unspecified()
                || a.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // unique local fc00::/7
                || (s[0] & 0xffc0) == 0xfe80 // link-local fe80::/10
                || (s[0] & 0xffc0) == 0xfec0 // site-local (deprecated)
                || (s[0] == 0x2001 && s[1] == 0x0db8) // documentation
                || (s[0] == 0x0064 && s[1] == 0xff9b) // NAT64: could reach any v4
                || s[0] == 0x2002 // 6to4: embeds a v4
                || (s[0] == 0x0100 && s[1..4] == [0, 0, 0])) // discard prefix
        }
    }
}

fn is_public_v4(a: Ipv4Addr) -> bool {
    let o = a.octets();
    !(a.is_unspecified()
        || a.is_loopback()
        || a.is_private()
        || a.is_link_local()
        || a.is_broadcast()
        || a.is_multicast()
        || a.is_documentation()
        || o[0] == 0 // 0.0.0.0/8
        || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64.0.0/10 carrier-grade NAT
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0.0/24
        || (o[0] == 198 && (o[1] & 0xfe) == 18) // 198.18.0.0/15 benchmarking
        || o[0] >= 240) // reserved
}

/// The pieces of `scheme://authority/...` the check needs: (scheme lower-cased, host, rest).
fn split(url: &str) -> Result<(String, &str), String> {
    let (scheme, rest) = url.split_once("://").ok_or("not an address (no scheme://)")?;
    let scheme = scheme.to_ascii_lowercase();
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    Ok((scheme, &rest[..end]))
}

/// Check a station address. `allow_http` lets `http://` through (see [`http_allowed`]).
/// The error says why, for the log and the radio display.
pub fn check_stream_url(url: &str, allow_http: bool) -> Result<(), String> {
    let url = url.trim();
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("the address has spaces or control characters".into());
    }
    let (scheme, authority) = split(url)?;
    match scheme.as_str() {
        "https" => {}
        "http" if allow_http => {}
        "http" => return Err("plain http is not allowed (set OMSI_RADIO_ALLOW_HTTP=1 to opt in); use an https address".into()),
        other => return Err(format!("the scheme {other:?} is not allowed (https only)")),
    }
    if authority.contains('@') {
        return Err("an address with user:password@ is not allowed".into());
    }
    // host and optional port; an IPv6 literal is in brackets
    let host = if let Some(rest) = authority.strip_prefix('[') {
        let (h, tail) = rest.split_once(']').ok_or("a broken IPv6 address")?;
        if !(tail.is_empty() || tail.starts_with(':')) {
            return Err("a broken IPv6 address".into());
        }
        h
    } else {
        authority.rsplit_once(':').map(|(h, port)| if port.chars().all(|c| c.is_ascii_digit()) { h } else { authority }).unwrap_or(authority)
    };
    if host.is_empty() {
        return Err("the address has no host".into());
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return if is_public_ip(ip) { Ok(()) } else { Err(format!("{ip} is not a public address")) };
    }
    let name = host.trim_end_matches('.').to_ascii_lowercase();
    if !name.contains('.') {
        return Err(format!("{host:?} is not a public host name"));
    }
    if name == "localhost" || [".localhost", ".local", ".internal", ".lan", ".home", ".home.arpa", ".intranet", ".corp"].iter().any(|s| name.ends_with(s)) {
        return Err(format!("{host:?} is not a public host name"));
    }
    // 2130706433, 0x7f.1, 127.1: numbers the system's resolver would read as an IPv4 address
    let last = name.rsplit('.').next().unwrap_or("");
    if last.chars().all(|c| c.is_ascii_digit()) || name.starts_with("0x") || name.contains(".0x") {
        return Err(format!("{host:?} looks like a numeric address"));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || !c.is_ascii()) {
        return Err(format!("{host:?} is not a valid host name"));
    }
    Ok(())
}

/// A resolver for `ureq` that returns only public addresses: a name that resolves to nothing
/// public (or to a mixture: the private ones are dropped) is refused at connect time.
#[derive(Debug, Clone, Copy, Default)]
pub struct PublicOnly;

impl ureq::Resolver for PublicOnly {
    fn resolve(&self, netloc: &str) -> std::io::Result<Vec<SocketAddr>> {
        let all: Vec<SocketAddr> = netloc.to_socket_addrs()?.collect();
        let public: Vec<SocketAddr> = all.iter().copied().filter(|a| is_public_ip(a.ip())).collect();
        if public.is_empty() && !all.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, format!("{netloc} does not resolve to a public address")));
        }
        Ok(public)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(u: &str) -> bool {
        check_stream_url(u, false).is_ok()
    }

    #[test]
    fn public_https_stations_pass() {
        assert!(ok("https://stream.radioparadise.com/mp3-128"));
        assert!(ok("https://nashe1.hostingradio.ru:80/nashe-128.mp3"));
        assert!(ok("HTTPS://Example.ORG/live.m3u?x=1#y"));
        assert!(ok("https://8.8.8.8/stream"));
        assert!(ok("https://[2606:4700:4700::1111]:443/stream"));
        assert!(ok("https://münchen.example.de/radio"));
    }

    #[test]
    fn plain_http_needs_the_opt_in() {
        assert!(check_stream_url("http://example.org/a.mp3", false).is_err());
        assert!(check_stream_url("http://example.org/a.mp3", true).is_ok());
        // the opt-in does not open the private ranges
        assert!(check_stream_url("http://192.168.1.1/a.mp3", true).is_err());
        assert!(check_stream_url("http://localhost:8080/a.mp3", true).is_err());
    }

    #[test]
    fn other_schemes_are_refused() {
        for u in ["file:///etc/passwd", "ftp://example.org/a", "gopher://example.org", "javascript:alert(1)", "data:audio/mp3;base64,AAAA", "example.org/a.mp3", "", "//example.org/a"] {
            assert!(check_stream_url(u, true).is_err(), "{u}");
        }
    }

    #[test]
    fn loopback_private_and_link_local_hosts_are_refused() {
        for u in [
            "https://127.0.0.1/a", "https://127.1.2.3:8080/a", "https://localhost/a", "https://LOCALHOST./a", "https://radio.localhost/a",
            "https://10.0.0.5/a", "https://172.16.0.1/a", "https://172.31.255.255/a", "https://192.168.0.1/a",
            "https://169.254.169.254/latest/meta-data", "https://0.0.0.0/a", "https://100.64.0.1/a", "https://224.0.0.1/a", "https://255.255.255.255/a",
            "https://[::1]/a", "https://[::]/a", "https://[fe80::1]/a", "https://[fd00::1]/a", "https://[fc00::1]:8080/a", "https://[::ffff:127.0.0.1]/a", "https://[::ffff:10.0.0.1]/a",
            "https://router/a", "https://nas.local/a", "https://printer.lan/a", "https://svc.internal/a", "https://2130706433/a", "https://0x7f.0.0.1/a", "https://127.1/a",
            "https://user:pass@example.org/a", "https://example.org@127.0.0.1/a", "https:///a", "https://[::1/a",
        ] {
            assert!(check_stream_url(u, true).is_err(), "{u} must be refused");
        }
    }

    #[test]
    fn public_and_non_public_addresses() {
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
        assert!(is_public_ip("2a00:1450:4001::1".parse().unwrap()));
        assert!(is_public_ip("::ffff:8.8.8.8".parse().unwrap()));
        for s in ["127.0.0.1", "10.1.1.1", "172.20.0.1", "192.168.5.5", "169.254.1.1", "0.1.2.3", "100.100.0.1", "198.18.0.1", "203.0.113.5", "240.0.0.1", "::1", "::", "fe80::abcd", "fd12::1", "ff02::1", "::ffff:192.168.1.1", "2001:db8::1", "64:ff9b::7f00:1"] {
            assert!(!is_public_ip(s.parse().unwrap()), "{s}");
        }
    }

    #[test]
    fn the_resolver_refuses_names_that_point_inside() {
        use ureq::Resolver;
        // a literal loopback "name" resolves to itself and is dropped
        assert!(PublicOnly.resolve("127.0.0.1:80").is_err());
        assert!(PublicOnly.resolve("[::1]:80").is_err());
        assert!(PublicOnly.resolve("192.168.1.1:443").is_err());
        let public = PublicOnly.resolve("8.8.8.8:443").unwrap();
        assert_eq!(public, vec!["8.8.8.8:443".parse().unwrap()]);
    }
}

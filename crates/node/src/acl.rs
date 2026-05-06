//! Per-tunnel source-IP access control for accepted connections.
//!
//! Rules:
//! - `deny` is checked first: any peer matching a deny CIDR is rejected.
//! - If `allow` is non-empty, the peer must match a CIDR there to be accepted.
//! - If `allow` is empty (and `deny` did not match), the peer is accepted —
//!   the default is "open", same as before this module existed.
//! - Invalid CIDR strings are logged and skipped at parse time, never causing
//!   a tunnel to fail to start.

use std::net::IpAddr;

use ipnet::IpNet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Acl {
    allow: Vec<IpNet>,
    deny: Vec<IpNet>,
}

impl Acl {
    pub fn new(allow: &[String], deny: &[String]) -> Self {
        Self {
            allow: parse_cidrs(allow, "allow"),
            deny: parse_cidrs(deny, "deny"),
        }
    }

    /// Returns true if a peer with the given IP should be accepted.
    pub fn permits(&self, peer: IpAddr) -> bool {
        // Dual-stack sockets present IPv4 clients as ::ffff:x.x.x.x; normalize
        // to plain IPv4 so ACL rules like "1.2.3.4/32" match correctly.
        let peer = match peer {
            IpAddr::V6(v6) => v6
                .to_ipv4_mapped()
                .map(IpAddr::V4)
                .unwrap_or(IpAddr::V6(v6)),
            v4 => v4,
        };
        if self.deny.iter().any(|n| n.contains(&peer)) {
            return false;
        }
        if self.allow.is_empty() {
            return true;
        }
        self.allow.iter().any(|n| n.contains(&peer))
    }
}

fn parse_cidrs(raw: &[String], side: &'static str) -> Vec<IpNet> {
    raw.iter()
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            // Bare IP without prefix — treat as /32 or /128.
            let parsed: Result<IpNet, String> = if s.contains('/') {
                s.parse::<IpNet>().map_err(|e| e.to_string())
            } else {
                s.parse::<IpAddr>()
                    .map(IpNet::from)
                    .map_err(|e| e.to_string())
            };
            match parsed {
                Ok(n) => Some(n),
                Err(e) => {
                    tracing::warn!(side, cidr = %s, error = %e, "ignoring invalid CIDR");
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn empty_allow_and_deny_permits_everything() {
        let acl = Acl::new(&[], &[]);
        assert!(acl.permits(ip("10.0.0.1")));
        assert!(acl.permits(ip("8.8.8.8")));
        assert!(acl.permits(ip("::1")));
    }

    #[test]
    fn allow_list_restricts_traffic() {
        let acl = Acl::new(&["10.0.0.0/8".into()], &[]);
        assert!(acl.permits(ip("10.0.0.1")));
        assert!(acl.permits(ip("10.255.255.255")));
        assert!(!acl.permits(ip("8.8.8.8")));
    }

    #[test]
    fn deny_overrides_allow() {
        let acl = Acl::new(&["10.0.0.0/8".into()], &["10.0.0.5/32".into()]);
        assert!(acl.permits(ip("10.0.0.1")));
        assert!(!acl.permits(ip("10.0.0.5")));
    }

    #[test]
    fn deny_alone_blocks_listed_subnets() {
        let acl = Acl::new(&[], &["192.168.0.0/16".into()]);
        assert!(!acl.permits(ip("192.168.1.7")));
        assert!(acl.permits(ip("8.8.8.8")));
    }

    #[test]
    fn bare_ip_treated_as_host_route() {
        let acl = Acl::new(&["1.2.3.4".into()], &[]);
        assert!(acl.permits(ip("1.2.3.4")));
        assert!(!acl.permits(ip("1.2.3.5")));
    }

    #[test]
    fn ipv6_supported() {
        let acl = Acl::new(&["2001:db8::/32".into()], &[]);
        assert!(acl.permits(ip("2001:db8::1")));
        assert!(!acl.permits(ip("2001:db9::1")));
    }

    #[test]
    fn invalid_entries_are_silently_skipped() {
        let acl = Acl::new(&["not-an-ip".into(), "10.0.0.0/8".into()], &[]);
        assert!(acl.permits(ip("10.0.0.1")));
        assert!(!acl.permits(ip("8.8.8.8")));
    }

    #[test]
    fn empty_allow_with_deny_match_rejects() {
        // Empty allow normally means "open", but an explicit deny match still
        // wins.
        let acl = Acl::new(&[], &["1.2.3.4/32".into()]);
        assert!(!acl.permits(ip("1.2.3.4")));
        assert!(acl.permits(ip("1.2.3.5")));
    }

    #[test]
    fn ipv4_mapped_ipv6_matches_ipv4_cidr() {
        // Dual-stack sockets present IPv4 peers as ::ffff:x.x.x.x; ensure
        // they still match IPv4 allow/deny rules.
        let acl = Acl::new(&["50.114.5.234/32".into()], &[]);
        assert!(acl.permits(ip("::ffff:50.114.5.234")));
        assert!(!acl.permits(ip("::ffff:50.114.5.235")));
    }
}

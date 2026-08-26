//! Which authorities this server answers for, and how a request states the
//! one it means.
//!
//! **A loopback bind is not an access control.** What binding `127.0.0.1`
//! decides is which *network* can open the socket; it says nothing about which
//! *page* gets to read the answer, and on this machine every browser can open
//! it. The rule that normally stops a page from reading a loopback response is
//! the same-origin policy, and DNS rebinding is the attack that dissolves
//! exactly that rule: a hostile page served from `attacker.example` whose name
//! resolves, on a second lookup, to `127.0.0.1` keeps its original origin while
//! its requests land here — so the browser treats the bundle this server
//! returns as same-origin with the attacker's page and hands it to script. The
//! served tree is somebody's document, translated; there is nothing in it that
//! is safe to publish to a page that asked for it under another name.
//!
//! **`Host` is what separates the two requests.** A rebound request carries
//! `Host: attacker.example`, never `127.0.0.1`, because the header mirrors the
//! URL the page used rather than the address the packet reached — the attacker
//! cannot change it without giving up the origin that made the attack worth
//! running. So every request must name an authority this server answers for,
//! and one that does not is refused before the served root is consulted.
//!
//! **What this is not.** It is not a defense against a local process, which can
//! open a socket and write any `Host` it likes; a local process is inside the
//! boundary `conn::resolve` already records. It is a defense against a
//! *browser* used as a confused deputy, which is the loopback default's real
//! exposure — and it is request validation rather than the "non-loopback
//! deployment hardening" ticket `b791d6` scopes out, since it costs one string
//! comparison and protects the default bind rather than a deployment.
//!
//! TRACE: SCN-13
//! TRACE: contracts.md §6

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

/// The port a client means when its `Host` states none. `http` has exactly one
/// default and this is it; a server on any other port is named with its port
/// or is not named at all.
const HTTP_DEFAULT_PORT: u16 = 80;

/// The host half of an authority.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Host {
    Ip(IpAddr),
    /// A registered name, ASCII-lowercased and with the root label's trailing
    /// dot removed, so the several spellings of one name compare as one value.
    Name(String),
}

/// A `host` or `host:port`, as a `Host` header states one and as `--allow-host`
/// takes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authority {
    host: Host,
    /// `None` when the text named no port. On a request that means the scheme
    /// default; in the allowlist it means the port this server bound, which is
    /// the only port an entry could have meant.
    port: Option<u16>,
}

/// What [`HostPolicy::verdict`] made of a request head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Exactly one `Host`, and it names this server.
    Answered,
    /// No `Host` field, more than one, or one that is not an authority. The
    /// request does not say who it is for, so there is nothing to check it
    /// against — `400`.
    Malformed,
    /// A well-formed authority that is not one this server answers for — `421`.
    Elsewhere,
}

/// The authorities one running server answers for.
#[derive(Debug, Clone)]
pub struct HostPolicy {
    /// Every accepted authority, port resolved. Small and fixed, so a linear
    /// scan is the whole lookup.
    answered: Vec<(Host, u16)>,
}

impl HostPolicy {
    /// What a server bound at `local` answers for, plus every `--allow-host`.
    ///
    /// The derived set is what a browser on this machine can reach the socket
    /// as, and nothing else:
    ///
    /// - **A specific address** answers for that address literal. When it is a
    ///   loopback one it also answers for `localhost`, the name RFC 6761
    ///   reserves for exactly this address and the name the smoke checklists
    ///   type — a name no attacker can point anywhere else.
    /// - **A wildcard bind** (`0.0.0.0`, `::`) names no interface, so nothing
    ///   can be derived from it but the loopback authorities it also listens
    ///   on. The address other machines reach it by is the operator's to state
    ///   with `--allow-host`; the alternative — answering for any name at all
    ///   the moment `--bind` is given — would leave the hole open in the one
    ///   configuration that includes loopback *and* is reachable from off the
    ///   machine.
    pub fn new(local: SocketAddr, allowed: &[Authority]) -> HostPolicy {
        let port = local.port();
        let ip = local.ip();
        let mut answered = Vec::with_capacity(3 + allowed.len());
        if ip.is_unspecified() {
            answered.push((Host::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST)), port));
            answered.push((Host::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST)), port));
            answered.push((Host::Name("localhost".to_string()), port));
        } else {
            answered.push((Host::Ip(ip), port));
            if ip.is_loopback() {
                answered.push((Host::Name("localhost".to_string()), port));
            }
        }
        for authority in allowed {
            answered.push((authority.host.clone(), authority.port.unwrap_or(port)));
        }
        HostPolicy { answered }
    }

    /// Judge a request head.
    ///
    /// `head` is the whole head, request line first, which is where the field
    /// count has to be taken: "exactly one `Host`" is a property of the message
    /// and cannot be recovered from one field's value.
    pub fn verdict(&self, head: &str) -> Verdict {
        let mut stated: Option<&str> = None;
        for line in head.lines().skip(1) {
            // An obs-fold continuation (RFC 9112 §5.2) makes "exactly one
            // Host" undecidable — a folded line either continues the field
            // above it or starts one, depending on who is reading — so a head
            // carrying one is not judged, it is refused.
            if line.starts_with(' ') || line.starts_with('\t') {
                return Verdict::Malformed;
            }
            // A field line with no colon is not a field (RFC 9112 §5), and
            // this parser is one of two that read this head — the browser is
            // the other. Skipping it (R0009-0005) meant judging a head some
            // conforming reader refuses, which is the parser disagreement the
            // obs-fold arm right above refuses to enter. Same posture, same
            // verdict.
            let Some((name, value)) = line.split_once(':') else {
                return Verdict::Malformed;
            };
            // `name` is taken verbatim: whitespace before the colon is not a
            // field name, so `Host : x` states no Host rather than a padded
            // one.
            if !name.eq_ignore_ascii_case("host") {
                continue;
            }
            if stated.is_some() {
                return Verdict::Malformed;
            }
            stated = Some(value);
        }
        let Some(stated) = stated else {
            return Verdict::Malformed;
        };
        let Ok(authority) = stated.parse::<Authority>() else {
            return Verdict::Malformed;
        };
        let port = authority.port.unwrap_or(HTTP_DEFAULT_PORT);
        if self
            .answered
            .iter()
            .any(|(host, bound)| *bound == port && *host == authority.host)
        {
            Verdict::Answered
        } else {
            Verdict::Elsewhere
        }
    }

    /// The authorities, as an operator would type them. Used both in the
    /// startup line and in the body of a refusal, so the answer to "why was
    /// that refused" is the same sentence in both places.
    pub fn answered_authorities(&self) -> String {
        self.answered
            .iter()
            .map(|(host, port)| format!("{host}:{port}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl FromStr for Authority {
    type Err = String;

    fn from_str(text: &str) -> Result<Authority, String> {
        parse(text).ok_or_else(|| format!("`{text}` is not a `host` or `host:port` authority"))
    }
}

/// `host`, `host:port`, `[v6]` or `[v6]:port` — the four shapes an authority
/// has. Anything else is not one, and a request that carries it is refused
/// rather than reinterpreted.
fn parse(text: &str) -> Option<Authority> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix('[') {
        let (inside, after) = rest.split_once(']')?;
        let addr = inside.parse::<Ipv6Addr>().ok()?;
        let port = match after {
            "" => None,
            stated => Some(port_from(stated.strip_prefix(':')?)?),
        };
        return Some(Authority {
            host: Host::Ip(IpAddr::V6(addr)),
            port,
        });
    }
    // One colon at most: a bare IPv6 address carries several and belongs in
    // brackets, so `::1` is refused rather than read as host `:` port `1`.
    let (name, port) = match text.split_once(':') {
        Some((name, stated)) => (name, Some(port_from(stated)?)),
        None => (text, None),
    };
    Some(Authority {
        host: host_from(name)?,
        port,
    })
}

/// Port 0 is a bind-time request for any free port, never an authority a
/// client can reach, so it is not a port here.
fn port_from(text: &str) -> Option<u16> {
    text.parse::<u16>().ok().filter(|port| *port != 0)
}

/// Longest a DNS name may be in its presentation form, root label dropped
/// (RFC 1035 §2.3.4's 255-octet wire limit, minus the length byte and the
/// root's terminating zero).
const MAX_NAME_LEN: usize = 253;

/// Longest one DNS label may be (RFC 1035 §2.3.4).
const MAX_LABEL_LEN: usize = 63;

/// Read `text` as an IPv4 literal or a registered name, or refuse it.
///
/// **The name shape is checked against what DNS can actually hold
/// (R0009-0007).** Requiring only nonempty LDH labels admitted `-example.test`,
/// `example-.test`, a 200-byte label and a 4 KB name — none of which is a
/// hostname any resolver will return, so accepting them let `--allow-host` and
/// a `Host` field agree on a string that names nothing. It grants nothing on
/// its own (a [`Host::Name`] only ever matters as an exact match against an
/// operator-supplied allowlist entry), which is precisely why the validation
/// should say what it means: a parser that accepts more than the grammar it
/// claims is a parser the next reader has to re-derive.
fn host_from(text: &str) -> Option<Host> {
    // The root label is implicit, so `localhost.` and `localhost` are one name.
    let text = text.strip_suffix('.').unwrap_or(text);
    if text.is_empty() {
        return None;
    }
    if let Ok(ip) = text.parse::<Ipv4Addr>() {
        return Some(Host::Ip(IpAddr::V4(ip)));
    }
    if text.len() > MAX_NAME_LEN {
        return None;
    }
    // Edge hyphens are refused: RFC 952, as relaxed by RFC 1123 §2.1, lets a
    // label begin with a letter or a digit and end with a letter or a digit —
    // a hyphen at either end is not a name any resolver returns.
    let is_label = |label: &str| {
        !label.is_empty()
            && label.len() <= MAX_LABEL_LEN
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    };
    if !text.split('.').all(is_label) {
        return None;
    }
    Some(Host::Name(text.to_ascii_lowercase()))
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // A v6 literal is bracketed wherever a port can follow it, which
            // is everywhere this prints.
            Host::Ip(IpAddr::V6(addr)) => write!(f, "[{addr}]"),
            Host::Ip(IpAddr::V4(addr)) => write!(f, "{addr}"),
            Host::Name(name) => write!(f, "{name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(bind: &str, allowed: &[&str]) -> HostPolicy {
        let allowed: Vec<Authority> = allowed
            .iter()
            .map(|text| text.parse().expect("test authorities should parse"))
            .collect();
        HostPolicy::new(bind.parse().expect("test bind should parse"), &allowed)
    }

    fn head(host_lines: &str) -> String {
        format!("GET /index.html HTTP/1.1\r\n{host_lines}")
    }

    fn verdict(policy: &HostPolicy, host_lines: &str) -> Verdict {
        policy.verdict(&head(host_lines))
    }

    #[test]
    fn an_authority_is_read_as_its_host_and_port() {
        let cases = [
            ("127.0.0.1:7470", "127.0.0.1", Some(7470)),
            ("localhost", "localhost", None),
            // Case and the root label's dot are spelling, not identity.
            ("LocalHost.", "localhost", None),
            ("[::1]:7470", "[::1]", Some(7470)),
            ("[::1]", "[::1]", None),
            ("demo.example:8080", "demo.example", Some(8080)),
        ];
        for (text, host, port) in cases {
            let authority: Authority = text.parse().unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(authority.host.to_string(), host, "{text}");
            assert_eq!(authority.port, port, "{text}");
        }
    }

    #[test]
    fn text_that_is_not_an_authority_is_refused_rather_than_guessed_at() {
        for text in [
            "",
            // A v6 literal without its brackets: several colons, no port.
            "::1",
            "127.0.0.1:",
            "127.0.0.1:0",
            "127.0.0.1:70000",
            "localhost:http",
            "local host",
            "[::1",
            "[not-an-address]",
            "example..com",
            "attacker.example/../",
            // R0009-0007: LDH characters alone are not a DNS name shape.
            "-example.test",
            "example-.test",
            "demo.-example.test",
        ] {
            assert!(
                text.parse::<Authority>().is_err(),
                "{text:?} is not an authority",
            );
        }
    }

    /// R0009-0007, the two limits that cannot be written as readable literals.
    /// Both are boundaries rather than magnitudes, so each is measured on
    /// either side of itself.
    #[test]
    fn a_dns_name_is_refused_past_the_label_and_name_limits() {
        let label = |n: usize| "a".repeat(n);
        assert!(
            format!("{}.test", label(MAX_LABEL_LEN))
                .parse::<Authority>()
                .is_ok(),
            "a {MAX_LABEL_LEN}-byte label is the longest DNS has",
        );
        assert!(
            format!("{}.test", label(MAX_LABEL_LEN + 1))
                .parse::<Authority>()
                .is_err(),
            "a {}-byte label is longer than DNS has",
            MAX_LABEL_LEN + 1,
        );

        // Four 63-byte labels joined by dots is 255 bytes; dropping two from
        // the last one lands exactly on the limit.
        let mut name = [label(63), label(63), label(63), label(61)].join(".");
        assert_eq!(name.len(), MAX_NAME_LEN);
        assert!(
            name.parse::<Authority>().is_ok(),
            "a {MAX_NAME_LEN}-byte name is the longest DNS has",
        );
        name.push('a');
        assert!(
            name.parse::<Authority>().is_err(),
            "a {}-byte name is longer than DNS has",
            MAX_NAME_LEN + 1,
        );
    }

    /// The default bind's whole allowlist: the address it bound and the one
    /// reserved name for it. Every other authority — including the same
    /// address on another port, and the loopback address it did *not* bind —
    /// is somewhere else.
    #[test]
    fn the_default_bind_answers_for_its_own_address_and_localhost() {
        let policy = policy("127.0.0.1:7470", &[]);
        for host in ["127.0.0.1:7470", "localhost:7470", "LOCALHOST:7470"] {
            assert_eq!(
                verdict(&policy, &format!("Host: {host}\r\n")),
                Verdict::Answered,
                "{host}",
            );
        }
        for host in [
            // The rebinding attack, in one line: a name the attacker owns,
            // pointed at this socket.
            "attacker.example:7470",
            "attacker.example",
            // Right host, wrong authority.
            "127.0.0.1:7471",
            "127.0.0.1",
            "[::1]:7470",
            "127.0.0.2:7470",
        ] {
            assert_eq!(
                verdict(&policy, &format!("Host: {host}\r\n")),
                Verdict::Elsewhere,
                "{host}",
            );
        }
    }

    /// A request that does not state exactly one authority states none: there
    /// is nothing to compare, so there is nothing to accept.
    #[test]
    fn a_head_that_does_not_state_one_authority_is_malformed() {
        let policy = policy("127.0.0.1:7470", &[]);
        assert_eq!(verdict(&policy, ""), Verdict::Malformed, "no Host at all");
        assert_eq!(
            verdict(&policy, "Connection: close\r\n"),
            Verdict::Malformed,
            "headers, but no Host",
        );
        // R0009-0005: a line with no colon is not a field, and a head that
        // carries one is judged by no two parsers alike — so it is not judged.
        assert_eq!(
            verdict(&policy, "Broken-Field\r\nHost: 127.0.0.1:7470\r\n"),
            Verdict::Malformed,
            "a colonless line makes the whole head malformed, Host or no Host",
        );
        assert_eq!(
            verdict(
                &policy,
                "Host: 127.0.0.1:7470\r\nHost: attacker.example\r\n"
            ),
            Verdict::Malformed,
            "two Host fields: whichever one is honored, some reader honors the other",
        );
        assert_eq!(
            verdict(
                &policy,
                "Host: attacker.example\r\nHost: 127.0.0.1:7470\r\n"
            ),
            Verdict::Malformed,
            "and in the other order",
        );
        assert_eq!(
            verdict(&policy, "Host: 127.0.0.1:7470\r\n\tattacker.example\r\n"),
            Verdict::Malformed,
            "an obs-fold continuation is not judged, it is refused",
        );
        assert_eq!(
            verdict(&policy, "Host : 127.0.0.1:7470\r\n"),
            Verdict::Malformed,
            "whitespace before the colon is not a field name",
        );
        assert_eq!(
            verdict(&policy, "Host: not an authority\r\n"),
            Verdict::Malformed,
        );
    }

    #[test]
    fn allow_host_widens_the_policy_and_nothing_else_does() {
        let policy = policy("192.168.1.5:7470", &["demo.example", "kiosk.example:9000"]);
        for host in [
            "192.168.1.5:7470",
            "demo.example:7470",
            "kiosk.example:9000",
        ] {
            assert_eq!(
                verdict(&policy, &format!("Host: {host}\r\n")),
                Verdict::Answered,
                "{host}",
            );
        }
        for host in [
            // A non-loopback bind is not reachable at loopback, so it does not
            // claim those names either.
            "localhost:7470",
            "127.0.0.1:7470",
            // An allowed name keeps the port it was given.
            "kiosk.example:7470",
            "demo.example:9000",
            "attacker.example:7470",
        ] {
            assert_eq!(
                verdict(&policy, &format!("Host: {host}\r\n")),
                Verdict::Elsewhere,
                "{host}",
            );
        }
    }

    /// A wildcard bind names no interface. What it can still derive is the
    /// loopback authorities it is also listening on — the ones the rebinding
    /// attack aims at — and the operator states the rest.
    #[test]
    fn a_wildcard_bind_answers_for_loopback_and_for_what_was_allowed() {
        let policy = policy("0.0.0.0:4319", &["192.168.1.5"]);
        for host in [
            "127.0.0.1:4319",
            "localhost:4319",
            "[::1]:4319",
            "192.168.1.5:4319",
        ] {
            assert_eq!(
                verdict(&policy, &format!("Host: {host}\r\n")),
                Verdict::Answered,
                "{host}",
            );
        }
        assert_eq!(
            verdict(&policy, "Host: attacker.example:4319\r\n"),
            Verdict::Elsewhere,
        );
    }

    #[test]
    fn the_authorities_print_the_way_they_are_typed() {
        assert_eq!(
            policy("127.0.0.1:7470", &["demo.example"]).answered_authorities(),
            "127.0.0.1:7470, localhost:7470, demo.example:7470",
        );
        assert_eq!(
            policy("[::1]:7470", &[]).answered_authorities(),
            "[::1]:7470, localhost:7470",
        );
    }
}

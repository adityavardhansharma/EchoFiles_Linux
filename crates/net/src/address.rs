//! Server addresses as people type them: `smb://nas/Media`, `\\nas\Media`, `me@host:/srv`,
//! `ftp://host:2121` — parsed into one shape, and printed back as a canonical URI (never
//! with a password in it).

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// Windows shares and most NAS boxes (Samba).
    Smb,
    /// Files over SSH.
    Sftp,
    Ftp,
    /// FTP upgraded to TLS (`AUTH TLS`, "explicit" FTPS).
    Ftps,
}

impl Protocol {
    pub const ALL: [Protocol; 4] = [Protocol::Smb, Protocol::Sftp, Protocol::Ftp, Protocol::Ftps];

    pub fn scheme(self) -> &'static str {
        match self {
            Protocol::Smb => "smb",
            Protocol::Sftp => "sftp",
            Protocol::Ftp => "ftp",
            Protocol::Ftps => "ftps",
        }
    }

    /// Short name for chips and menus.
    pub fn label(self) -> &'static str {
        match self {
            Protocol::Smb => "SMB",
            Protocol::Sftp => "SFTP",
            Protocol::Ftp => "FTP",
            Protocol::Ftps => "FTPS",
        }
    }

    /// What it is, for people who don't know the acronym.
    pub fn describe(self) -> &'static str {
        match self {
            Protocol::Smb => "Windows share",
            Protocol::Sftp => "SSH server",
            Protocol::Ftp => "FTP server",
            Protocol::Ftps => "FTP server (TLS)",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Protocol::Smb => 445,
            Protocol::Sftp => 22,
            Protocol::Ftp | Protocol::Ftps => 21,
        }
    }

    pub fn from_scheme(s: &str) -> Option<Protocol> {
        Some(match s.to_ascii_lowercase().as_str() {
            "smb" | "cifs" | "samba" => Protocol::Smb,
            "sftp" | "ssh" | "scp" => Protocol::Sftp,
            "ftp" => Protocol::Ftp,
            "ftps" | "ftpes" => Protocol::Ftps,
            _ => return None,
        })
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A server, and optionally a folder on it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Address {
    pub protocol: Protocol,
    /// Host name or IP; IPv6 without brackets.
    pub host: String,
    pub port: Option<u16>,
    pub user: Option<String>,
    /// SMB workgroup or domain (`smb://DOMAIN;user@host/share`).
    pub domain: Option<String>,
    /// SMB share. `None` on SMB means "show me the shares".
    pub share: Option<String>,
    /// Folder inside the share (SMB) or on the server, `/`-separated, no leading slash.
    pub path: String,
}

impl Address {
    /// Parse what someone typed. `default` is used when the text names no protocol
    /// (`nas.local/Media` with SMB picked in the dialog).
    pub fn parse(input: &str, default: Protocol) -> Result<Address, String> {
        let s = input.trim();
        if s.is_empty() {
            return Err("Type a server address".into());
        }
        // Windows UNC: \\nas\Media\Photos
        if let Some(rest) = s.strip_prefix("\\\\") {
            return Address::parse_authority_path(Protocol::Smb, &rest.replace('\\', "/"));
        }
        if let Some((scheme, rest)) = s.split_once("://") {
            let protocol = Protocol::from_scheme(scheme).ok_or_else(|| format!("EchoFiles can't connect with “{scheme}://”. Use smb://, sftp://, ftp:// or ftps://"))?;
            return Address::parse_authority_path(protocol, rest);
        }
        // //nas/Media (UNC with forward slashes)
        if let Some(rest) = s.strip_prefix("//") {
            return Address::parse_authority_path(Protocol::Smb, rest);
        }
        // scp style: me@host:/srv/files or host:/srv
        if let Some((authority, path)) = s.split_once(":/")
            && !authority.contains('/') && !authority.contains('[') && !authority.is_empty() && authority.rsplit_once(':').is_none() {
                return Address::parse_authority_path(Protocol::Sftp, &format!("{authority}/{path}"));
            }
        Address::parse_authority_path(default, s)
    }

    /// `[user@]host[:port][/path]`, already without the scheme.
    fn parse_authority_path(protocol: Protocol, rest: &str) -> Result<Address, String> {
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None => (rest, ""),
        };
        let (userinfo, hostport) = match authority.rfind('@') {
            Some(i) => (Some(&authority[..i]), &authority[i + 1..]),
            None => (None, authority),
        };
        let (host, port) = split_host_port(hostport)?;
        if host.is_empty() {
            return Err("The address needs a server name or IP".into());
        }
        if host.chars().any(|c| c.is_whitespace() || matches!(c, '\\' | '"' | '<' | '>' | '|' | '?' | '*')) {
            return Err(format!("“{host}” isn't a server name"));
        }
        let (mut user, mut domain) = (None, None);
        if let Some(info) = userinfo {
            // Never keep a password from the address; the login dialog asks for it.
            let info = info.split(':').next().unwrap_or("");
            let info = decode(info);
            let (d, u) = match info.split_once(';') {
                Some((d, u)) if protocol == Protocol::Smb => (Some(d.to_string()), u.to_string()),
                _ => match info.split_once('\\') {
                    Some((d, u)) if protocol == Protocol::Smb => (Some(d.to_string()), u.to_string()),
                    _ => (None, info),
                },
            };
            domain = d.filter(|d| !d.is_empty());
            user = Some(u).filter(|u| !u.is_empty());
        }
        let path = decode(path);
        let mut parts = path.split('/').filter(|p| !p.is_empty() && *p != ".");
        let share = if protocol == Protocol::Smb { parts.next().map(str::to_string) } else { None };
        let path = parts.collect::<Vec<_>>().join("/");
        Ok(Address { protocol, host: host.to_string(), port, user, domain, share, path })
    }

    /// `smb://DOMAIN;me@nas:4445/Media/Photos` — no password, ever.
    pub fn uri(&self) -> String {
        let mut s = format!("{}://", self.protocol.scheme());
        if let Some(u) = &self.user {
            if let Some(d) = &self.domain {
                s.push_str(&encode(d));
                s.push(';');
            }
            s.push_str(&encode(u));
            s.push('@');
        }
        if self.host.contains(':') {
            s.push('[');
            s.push_str(&self.host);
            s.push(']');
        } else {
            s.push_str(&self.host);
        }
        if let Some(p) = self.port {
            s.push_str(&format!(":{p}"));
        }
        s.push('/');
        if let Some(sh) = &self.share {
            s.push_str(&encode(sh));
            s.push('/');
        }
        if !self.path.is_empty() {
            for (i, part) in self.path.split('/').enumerate() {
                if i > 0 {
                    s.push('/');
                }
                s.push_str(&encode(part));
            }
        }
        s
    }

    /// The server alone, for the sidebar: the same address without the folder path.
    pub fn server(&self) -> Address {
        Address { path: String::new(), ..self.clone() }
    }

    /// "Media on nas.local", "me on build-box", "ftp.example.com".
    pub fn display_name(&self) -> String {
        match (&self.share, &self.user) {
            (Some(share), _) => format!("{share} on {}", self.host),
            (None, Some(u)) if self.protocol != Protocol::Smb => format!("{u} on {}", self.host),
            _ => self.host.clone(),
        }
    }

    /// The short sidebar name: the share, else the host ("Media", "build-box").
    pub fn short_name(&self) -> String {
        self.share.clone().unwrap_or_else(|| self.host.clone())
    }

    /// `me@nas.local:4445` — the port only when it isn't the protocol's usual one.
    pub fn who_where(&self) -> String {
        match &self.user {
            Some(u) => format!("{u}@{}", self.host_port()),
            None => self.host_port(),
        }
    }

    /// `nas.local:4445` when the port isn't the protocol's usual one.
    pub fn host_port(&self) -> String {
        match self.port {
            Some(p) if p != self.protocol.default_port() => format!("{}:{p}", self.host),
            _ => self.host.clone(),
        }
    }
}

fn split_host_port(s: &str) -> Result<(&str, Option<u16>), String> {
    if let Some(rest) = s.strip_prefix('[') {
        let end = rest.find(']').ok_or("Missing “]” after the IPv6 address")?;
        let host = &rest[..end];
        let after = &rest[end + 1..];
        return match after.strip_prefix(':') {
            Some(p) => Ok((host, Some(parse_port(p)?))),
            None if after.is_empty() => Ok((host, None)),
            None => Err(format!("Unexpected “{after}” after the address")),
        };
    }
    match s.rsplit_once(':') {
        // A bare IPv6 address without brackets has several colons: all host.
        Some((h, p)) if !h.contains(':') => Ok((h, if p.is_empty() { None } else { Some(parse_port(p)?) })),
        _ => Ok((s, None)),
    }
}

fn parse_port(p: &str) -> Result<u16, String> {
    match p.parse::<u16>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("“{p}” isn't a port number (1–65535)")),
    }
}

/// Percent-decoding (lossy on bad UTF-8).
pub fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encoding for one URI segment.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~!$&'()*+,=".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Address {
        Address::parse(s, Protocol::Smb).unwrap()
    }

    #[test]
    fn smb_forms() {
        let a = p("smb://nas.local/Media/Photos/2024");
        assert_eq!((a.host.as_str(), a.share.as_deref(), a.path.as_str()), ("nas.local", Some("Media"), "Photos/2024"));
        assert_eq!(p(r"\\nas\Media\Photos"), p("smb://nas/Media/Photos"));
        assert_eq!(p("//nas/Media"), p("smb://nas/Media"));
        let a = p("smb://WORK;alice@10.0.0.2:4445/Team%20Files/");
        assert_eq!((a.domain.as_deref(), a.user.as_deref(), a.port, a.share.as_deref()), (Some("WORK"), Some("alice"), Some(4445), Some("Team Files")));
        assert_eq!(a.uri(), "smb://WORK;alice@10.0.0.2:4445/Team%20Files/");
        assert_eq!(p("smb://nas").share, None);
        assert_eq!(p("nas.local/Media").share.as_deref(), Some("Media"));
    }

    #[test]
    fn sftp_ftp_forms() {
        let a = p("me@build-box:/srv/www");
        assert_eq!((a.protocol, a.user.as_deref(), a.host.as_str(), a.path.as_str()), (Protocol::Sftp, Some("me"), "build-box", "srv/www"));
        let a = p("ssh://me:secret@host:2222/home/me");
        assert_eq!((a.protocol, a.port, a.user.as_deref()), (Protocol::Sftp, Some(2222), Some("me")));
        assert!(!a.uri().contains("secret"));
        let a = p("ftp://[::1]:2121/pub");
        assert_eq!((a.host.as_str(), a.port, a.path.as_str()), ("::1", Some(2121), "pub"));
        assert_eq!(a.uri(), "ftp://[::1]:2121/pub");
        assert_eq!(Address::parse("files.example.com", Protocol::Ftp).unwrap().protocol, Protocol::Ftp);
    }

    #[test]
    fn rejects_nonsense() {
        assert!(Address::parse("", Protocol::Smb).is_err());
        assert!(Address::parse("http://x", Protocol::Smb).is_err());
        assert!(Address::parse("smb://host:99999/x", Protocol::Smb).is_err());
        assert!(Address::parse("smb:///share", Protocol::Smb).is_err());
    }

    #[test]
    fn names() {
        assert_eq!(p("smb://nas/Media").display_name(), "Media on nas");
        assert_eq!(p("sftp://me@box").display_name(), "me on box");
        assert_eq!(p("ftp://host:21").host_port(), "host");
        assert_eq!(p("ftp://host:2121").host_port(), "host:2121");
    }
}

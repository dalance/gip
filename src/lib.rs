extern crate core;
#[macro_use]
extern crate error_chain;
extern crate hyper;
extern crate rand;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate time;
extern crate toml;

use core::str::FromStr;
use hyper::Client;
use hyper::header::Connection;
use time::Tm;
use rand::{thread_rng, Rng};
use std::io::Read;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// -------------------------------------------------------------------------------------------------
// Default providers
// -------------------------------------------------------------------------------------------------

pub static DEFAULT_TOML: &'static str = r#"
    [[providers]]
        name    = "inet-ip.info"
        ptype   = "IPv4"
        format  = "Plane"
        url     = "http://inet-ip.info/ip"
        key     = []

    [[providers]]
        name    = "httpbin.org"
        ptype   = "IPv4"
        format  = "Json"
        url     = "http://httpbin.org/ip"
        key     = ["origin"]

    [[providers]]
        name    = "ipify.org"
        ptype   = "IPv4"
        format  = "Plane"
        url     = "http://api.ipify.org"
        key     = []

    [[providers]]
        name    = "freegeoip"
        ptype   = "IPv4"
        format  = "Json"
        url     = "http://freegeoip.net/json"
        key     = ["ip"]

    [[providers]]
        name    = "ipv6-test"
        ptype   = "IPv4"
        format  = "Plane"
        url     = "http://v4.ipv6-test.com/api/myip.php"
        key     = []

    [[providers]]
        name    = "ipv6-test"
        ptype   = "IPv6"
        format  = "Plane"
        url     = "http://v6.ipv6-test.com/api/myip.php"
        key     = []

    [[providers]]
        name    = "ident.me"
        ptype   = "IPv4"
        format  = "Plane"
        url     = "http://ident.me/"
        key     = []

    [[providers]]
        name    = "ident.me"
        ptype   = "IPv6"
        format  = "Plane"
        url     = "http://v6.ident.me/"
        key     = []
"#;

// -------------------------------------------------------------------------------------------------
// Error
// -------------------------------------------------------------------------------------------------

error_chain! {
    foreign_links {
        AddrParse(::std::net::AddrParseError);
        JsonParse(::serde_json::Error);
        Hyper(::hyper::Error);
        Toml(::toml::de::Error);
    }
    errors {
        GetAddressFailed {
            description("get address failed")
            display("failed to get address")
        }
    }
}

// -------------------------------------------------------------------------------------------------
// GlobalAddress
// -------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct GlobalAddress {
    /// Time of checking address
    pub time: Tm,
    /// Global IP address by IPv4
    pub v4addr: Option<Ipv4Addr>,
    /// Global IP address by IPv6
    pub v6addr: Option<Ipv6Addr>,
    /// Provider name used for checking address
    pub provider: String,
}

impl GlobalAddress {
    pub fn from_v4(addr: Ipv4Addr, provider: &str) -> Self {
        GlobalAddress {
            time: time::now(),
            v4addr: Some(addr),
            v6addr: None,
            provider: String::from(provider),
        }
    }

    pub fn from_v6(addr: Ipv6Addr, provider: &str) -> Self {
        GlobalAddress {
            time: time::now(),
            v4addr: None,
            v6addr: Some(addr),
            provider: String::from(provider),
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Provider
// -------------------------------------------------------------------------------------------------

pub trait Provider {
    /// Get global IP address
    fn get_addr(&mut self) -> Result<GlobalAddress>;
    /// Get provider name
    fn get_name(&self) -> String;
    /// Get provider type
    fn get_type(&self) -> ProviderType;
    /// Set timeout by milliseconds
    fn set_timeout(&mut self, timeout: usize);
    /// Set proxy
    fn set_proxy(&mut self, host: &str, port: u16);
}

// -------------------------------------------------------------------------------------------------
// ProviderInfo
// -------------------------------------------------------------------------------------------------

/// Type of global address from provider
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub enum ProviderType {
    IPv4,
    IPv6,
}

/// Format of return value from provider
#[derive(Debug, Deserialize)]
pub enum ProviderFormat {
    /// Plane text format
    Plane,
    /// JSON format
    Json,
}

#[derive(Debug, Deserialize)]
pub struct ProviderInfo {
    /// Provider name
    pub name: String,
    /// Provider type
    pub ptype: ProviderType,
    /// Provider format
    pub format: ProviderFormat,
    /// URL for GET
    pub url: String,
    /// Key for JSON format
    pub key: Vec<String>,
}

/// Provider information.
///
/// # Examples
/// ```
/// use gip::{ProviderFormat, ProviderInfo, ProviderType};
/// let p = ProviderInfo::new()
///     .name("inet-ip.info")
///     .ptype(ProviderType::IPv4)
///     .format(ProviderFormat::Plane)
///     .url("http://inet-ip.info/ip")
///     .key(&vec![]);
/// println!("{:?}", p);
/// ```
impl ProviderInfo {
    pub fn new() -> Self {
        ProviderInfo {
            name: String::from(""),
            ptype: ProviderType::IPv4,
            format: ProviderFormat::Plane,
            url: String::from(""),
            key: Vec::new(),
        }
    }

    pub fn name(self, name: &str) -> Self {
        ProviderInfo {
            name: String::from(name),
            ptype: self.ptype,
            format: self.format,
            url: self.url,
            key: self.key,
        }
    }

    pub fn ptype(self, ptype: ProviderType) -> Self {
        ProviderInfo {
            name: self.name,
            ptype: ptype,
            format: self.format,
            url: self.url,
            key: self.key,
        }
    }

    pub fn format(self, format: ProviderFormat) -> Self {
        ProviderInfo {
            name: self.name,
            ptype: self.ptype,
            format: format,
            url: self.url,
            key: self.key,
        }
    }

    pub fn url(self, url: &str) -> Self {
        ProviderInfo {
            name: self.name,
            ptype: self.ptype,
            format: self.format,
            url: String::from(url),
            key: self.key,
        }
    }

    pub fn key(self, key: &Vec<String>) -> Self {
        ProviderInfo {
            name: self.name,
            ptype: self.ptype,
            format: self.format,
            url: self.url,
            key: key.clone(),
        }
    }

    /// Create `Provider` from this info
    pub fn create(self) -> Box<Provider> {
        match self.format {
            ProviderFormat::Plane => {
                let mut p = Box::new(ProviderPlane::new());
                p.info = self;
                p
            }
            ProviderFormat::Json => {
                let mut p = Box::new(ProviderJson::new());
                p.info = self;
                p
            }
        }
    }
}

// -------------------------------------------------------------------------------------------------
// ProviderList
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ProviderList {
    /// Provider list
    pub providers: Vec<ProviderInfo>,
}

impl ProviderList {
    /// Load provider info from TOML string
    pub fn from_toml(s: &str) -> Result<ProviderList> {
        let t: ProviderList = toml::from_str(s).chain_err(|| "failed to parse provider list")?;
        Ok(t)
    }
}

// -------------------------------------------------------------------------------------------------
// ProviderAny
// -------------------------------------------------------------------------------------------------

/// Provider for checking global address from multiple providers
pub struct ProviderAny {
    /// Providers for checking global address
    pub providers: Vec<Box<Provider>>,
    /// Provider type
    pub ptype: ProviderType,
}

impl ProviderAny {
    pub fn new() -> Self {
        ProviderAny {
            providers: Vec::new(),
            ptype: ProviderType::IPv4,
        }
    }

    /// Load providers from TOML string
    pub fn from_toml(s: &str) -> Result<Self> {
        let list = ProviderList::from_toml(s)?;
        let mut p = Vec::new();
        for l in list.providers {
            p.push(l.create());
        }

        let ret = ProviderAny {
            providers: p,
            ptype: ProviderType::IPv4,
        };
        Ok(ret)
    }
}

impl Provider for ProviderAny {
    fn get_addr(&mut self) -> Result<GlobalAddress> {
        let mut rng = thread_rng();
        rng.shuffle(&mut self.providers);

        for p in &mut self.providers {
            if p.get_type() == self.ptype {
                let ret = p.get_addr();
                if ret.is_ok() {
                    return ret;
                }
            }
        }
        bail!(ErrorKind::GetAddressFailed)
    }

    fn get_name(&self) -> String {
        String::from("any")
    }

    fn get_type(&self) -> ProviderType {
        self.ptype.clone()
    }

    fn set_timeout(&mut self, timeout: usize) {
        for p in &mut self.providers {
            p.set_timeout(timeout)
        }
    }

    fn set_proxy(&mut self, host: &str, port: u16) {
        for p in &mut self.providers {
            p.set_proxy(host, port)
        }
    }
}

// -------------------------------------------------------------------------------------------------
// ProviderPlane
// -------------------------------------------------------------------------------------------------

/// Provider for checking global address by plane text format.
///
/// # Examples
/// ```
/// use gip::{Provider, ProviderInfo, ProviderPlane};
/// let mut p = ProviderInfo::new()
///     .url("http://inet-ip.info/ip")
///     .create();
/// let addr = p.get_addr().unwrap();
/// println!( "{:?}", addr.v4addr );
/// ```
pub struct ProviderPlane {
    /// Provider info
    pub info: ProviderInfo,
    /// Timeout
    pub timeout: usize,
    /// Proxy
    pub proxy: Option<(String, u16)>,
}

impl ProviderPlane {
    pub fn new() -> Self {
        ProviderPlane {
            info: ProviderInfo::new(),
            timeout: 1000,
            proxy: None,
        }
    }
}

impl Provider for ProviderPlane {
    fn get_addr(&mut self) -> Result<GlobalAddress> {
        let (tx, rx) = mpsc::channel();

        let url = self.info.url.clone();
        let proxy = self.proxy.clone();

        thread::spawn(move || {
            let client = match proxy {
                Some((x, y)) => Client::with_http_proxy(x, y),
                None => Client::new(),
            };
            let res = client.get(&url).header(Connection::close()).send();
            let _ = tx.send(res);
        });

        let mut cnt = 0;
        loop {
            match rx.try_recv() {
                Ok(x) => {
                    let mut body = String::new();
                    let _ = x?.read_to_string(&mut body);

                    let ret = match self.info.ptype {
                        ProviderType::IPv4 => {
                            let addr = Ipv4Addr::from_str(body.trim())?;
                            GlobalAddress::from_v4(addr, &self.info.name)
                        }
                        ProviderType::IPv6 => {
                            let addr = Ipv6Addr::from_str(body.trim())?;
                            GlobalAddress::from_v6(addr, &self.info.name)
                        }
                    };

                    return Ok(ret);
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(100));
                    cnt += 1;
                    if cnt > self.timeout / 100 {
                        bail!(ErrorKind::GetAddressFailed)
                    }
                }
            }
        }
    }

    fn get_name(&self) -> String {
        self.info.name.clone()
    }

    fn get_type(&self) -> ProviderType {
        self.info.ptype.clone()
    }

    fn set_timeout(&mut self, timeout: usize) {
        self.timeout = timeout
    }

    fn set_proxy(&mut self, host: &str, port: u16) {
        self.proxy = Some((String::from(host), port))
    }
}

// -------------------------------------------------------------------------------------------------
// ProviderJson
// -------------------------------------------------------------------------------------------------

/// Provider for checking global address by JSON format.
///
/// # Examples
/// ```
/// use gip::{ProviderFormat, ProviderInfo};
/// let mut p = ProviderInfo::new()
///     .format(ProviderFormat::Json)
///     .url("http://httpbin.org/ip")
///     .key(&vec!["origin".to_string()])
///     .create();
/// let addr = p.get_addr().unwrap();
/// println!( "{:?}", addr.v4addr );
/// ```
pub struct ProviderJson {
    /// Provider info
    pub info: ProviderInfo,
    /// Timeout
    pub timeout: usize,
    /// Proxy
    pub proxy: Option<(String, u16)>,
}

impl ProviderJson {
    pub fn new() -> Self {
        ProviderJson {
            info: ProviderInfo::new(),
            timeout: 1000,
            proxy: None,
        }
    }
}

impl Provider for ProviderJson {
    fn get_addr(&mut self) -> Result<GlobalAddress> {
        let (tx, rx) = mpsc::channel();

        let url = self.info.url.clone();
        let proxy = self.proxy.clone();

        thread::spawn(move || {
            let client = match proxy {
                Some((x, y)) => Client::with_http_proxy(x, y),
                None => Client::new(),
            };
            let res = client.get(&url).header(Connection::close()).send();
            let _ = tx.send(res);
        });

        let mut cnt = 0;
        loop {
            match rx.try_recv() {
                Ok(x) => {
                    let mut body = String::new();
                    let _ = x?.read_to_string(&mut body);
                    let json: serde_json::Value = serde_json::from_str(&body)?;
                    let key = format!("/{}", self.info.key.join("/"));
                    let addr = json.pointer(&key).unwrap().as_str().unwrap();

                    let ret = match self.info.ptype {
                        ProviderType::IPv4 => {
                            let addr = Ipv4Addr::from_str(addr)?;
                            GlobalAddress::from_v4(addr, &self.info.name)
                        }
                        ProviderType::IPv6 => {
                            let addr = Ipv6Addr::from_str(addr)?;
                            GlobalAddress::from_v6(addr, &self.info.name)
                        }
                    };

                    return Ok(ret);
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(100));
                    cnt += 1;
                    if cnt > self.timeout / 100 {
                        bail!(ErrorKind::GetAddressFailed)
                    }
                }
            }
        }
    }

    fn get_name(&self) -> String {
        self.info.name.clone()
    }

    fn get_type(&self) -> ProviderType {
        self.info.ptype.clone()
    }

    fn set_timeout(&mut self, timeout: usize) {
        self.timeout = timeout
    }

    fn set_proxy(&mut self, host: &str, port: u16) {
        self.proxy = Some((String::from(host), port))
    }
}

// -------------------------------------------------------------------------------------------------
// Test
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests_v4 {
    use super::*;

    #[test]
    fn inet_ip() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderType::IPv4)
            .format(ProviderFormat::Plane)
            .url("http://inet-ip.info/ip")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn httpbin() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderType::IPv4)
            .format(ProviderFormat::Json)
            .url("http://httpbin.org/ip")
            .key(&vec![String::from("origin")])
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn ipify() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderType::IPv4)
            .format(ProviderFormat::Plane)
            .url("http://api.ipify.org")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn freegeoip() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderType::IPv4)
            .format(ProviderFormat::Json)
            .url("http://freegeoip.net/json")
            .key(&vec![String::from("ip")])
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn ipv6_test() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderType::IPv4)
            .format(ProviderFormat::Plane)
            .url("http://v4.ipv6-test.com/api/myip.php")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn ident_me() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderType::IPv4)
            .format(ProviderFormat::Plane)
            .url("http://ident.me")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn toml_load() {
        let _ = ProviderList::from_toml(&DEFAULT_TOML);
    }

    #[test]
    fn provider_any() {
        let mut p0 = ProviderAny::from_toml(&DEFAULT_TOML).unwrap();
        let addr = p0.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }
}

#[cfg(test)]
mod tests_v6 {
    use super::*;

    #[test]
    fn ipv6_test() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderType::IPv6)
            .format(ProviderFormat::Plane)
            .url("http://v6.ipv6-test.com/api/myip.php")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v6addr.is_some());
    }

    #[test]
    fn ident_me() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderType::IPv6)
            .format(ProviderFormat::Plane)
            .url("http://v6.ident.me")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v6addr.is_some());
    }

}

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
use std::net::Ipv4Addr;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// -------------------------------------------------------------------------------------------------
// Default providers
// -------------------------------------------------------------------------------------------------

pub static DEFAULT_TOML: &'static str = r#"
    [[providers]]
        name    = "inet-ip.info"
        ptype   = "Plane"
        timeout = 1000
        url     = "http://inet-ip.info/ip"
        key     = []

    [[providers]]
        name    = "httpbin.org"
        ptype   = "Json"
        timeout = 1000
        url     = "http://httpbin.org/ip"
        key     = ["origin"]

    [[providers]]
        name    = "ipify.org"
        ptype   = "Plane"
        timeout = 1000
        url     = "http://api.ipify.org"
        key     = []

    [[providers]]
        name    = "freegeoip"
        ptype   = "Json"
        timeout = 1000
        url     = "http://freegeoip.net/json"
        key     = ["ip"]
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
    pub addr: Option<Ipv4Addr>,
    /// Provider name used for checking address
    pub provider: String,
}

// -------------------------------------------------------------------------------------------------
// Provider
// -------------------------------------------------------------------------------------------------

pub trait Provider {
    /// Get global IP address
    fn get_addr(&mut self) -> Result<GlobalAddress>;
    /// Get provider name
    fn get_name(&self) -> String;
    /// Set timeout by milliseconds
    fn set_timeout(&mut self, timeout: usize);
    /// Set proxy
    fn set_proxy(&mut self, host: &str, port: u16);
}

// -------------------------------------------------------------------------------------------------
// ProviderInfo
// -------------------------------------------------------------------------------------------------

/// Format of return value from provider
#[derive(Debug, Deserialize)]
pub enum ProviderType {
    /// Plane text format
    Plane,
    /// JSON format
    Json,
}

#[derive(Debug, Deserialize)]
pub struct ProviderInfo {
    /// Provider name
    pub name: String,
    /// Provider format
    pub ptype: ProviderType,
    /// Timeout
    pub timeout: usize,
    /// URL for GET
    pub url: String,
    /// Key for JSON format
    pub key: Vec<String>,
}

impl ProviderInfo {
    pub fn new() -> Self {
        ProviderInfo {
            name: String::from(""),
            ptype: ProviderType::Plane,
            timeout: 1000,
            url: String::from(""),
            key: Vec::new(),
        }
    }

    /// Create `Provider` from this info
    pub fn create(&self) -> Box<Provider> {
        match self.ptype {
            ProviderType::Plane => {
                let mut p = Box::new(ProviderPlane::new());
                p.name = self.name.clone();
                p.timeout = self.timeout;
                p.url = self.url.clone();
                p
            }
            ProviderType::Json => {
                let mut p = Box::new(ProviderJson::new());
                p.name = self.name.clone();
                p.timeout = self.timeout;
                p.url = self.url.clone();
                p.key = self.key.clone();
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
}

impl ProviderAny {
    pub fn new() -> Self {
        ProviderAny {
            providers: Vec::new(),
        }
    }

    /// Load providers from TOML string
    pub fn from_toml(s: &str) -> Result<Self> {
        let list = ProviderList::from_toml(s)?;
        let mut p = Vec::new();
        for l in list.providers {
            p.push(l.create());
        }

        let ret = ProviderAny { providers: p };
        Ok(ret)
    }
}

impl Provider for ProviderAny {
    fn get_addr(&mut self) -> Result<GlobalAddress> {
        let mut rng = thread_rng();
        rng.shuffle(&mut self.providers);

        for p in &mut self.providers {
            let ret = p.get_addr();
            if ret.is_ok() {
                return ret;
            }
        }
        bail!(ErrorKind::GetAddressFailed)
    }

    fn get_name(&self) -> String {
        String::from("any")
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
/// use gip::{Provider, ProviderPlane};
/// let mut p = ProviderPlane::new();
/// p.url = String::from( "http://inet-ip.info/ip" );
/// let addr = p.get_addr().unwrap();
/// println!( "{:?}", addr.addr );
/// ```
pub struct ProviderPlane {
    /// Provider name
    pub name: String,
    /// URL for GET
    pub url: String,
    /// Timeout
    pub timeout: usize,
    /// Proxy
    pub proxy: Option<(String, u16)>,
}

impl ProviderPlane {
    pub fn new() -> Self {
        ProviderPlane {
            name: String::new(),
            url: String::new(),
            timeout: 1000,
            proxy: None,
        }
    }
}

impl Provider for ProviderPlane {
    fn get_addr(&mut self) -> Result<GlobalAddress> {
        let (tx, rx) = mpsc::channel();

        let name = self.name.clone();
        let url = self.url.clone();
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
                    let addr = Ipv4Addr::from_str(body.trim())?;

                    let ret = GlobalAddress {
                        time: time::now(),
                        addr: Some(addr),
                        provider: name,
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
        self.name.clone()
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
/// use gip::{Provider, ProviderJson};
/// let mut p = ProviderJson::new();
/// p.url = String::from( "http://httpbin.org/ip" );
/// p.key = vec!["origin".to_string()];
/// let addr = p.get_addr().unwrap();
/// println!( "{:?}", addr.addr );
/// ```
pub struct ProviderJson {
    /// Provider name
    pub name: String,
    /// URL for GET
    pub url: String,
    /// Key for JSON format
    pub key: Vec<String>,
    /// Timeout
    pub timeout: usize,
    /// Proxy
    pub proxy: Option<(String, u16)>,
}

impl ProviderJson {
    pub fn new() -> Self {
        ProviderJson {
            name: String::new(),
            url: String::new(),
            key: Vec::new(),
            timeout: 1000,
            proxy: None,
        }
    }
}

impl Provider for ProviderJson {
    fn get_addr(&mut self) -> Result<GlobalAddress> {
        let (tx, rx) = mpsc::channel();

        let name = self.name.clone();
        let url = self.url.clone();
        let key = self.key.clone();
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
                    let key = format!("/{}", key.join("/"));
                    let addr = json.pointer(&key).unwrap().as_str().unwrap();
                    let addr = Ipv4Addr::from_str(addr)?;

                    let ret = GlobalAddress {
                        time: time::now(),
                        addr: Some(addr),
                        provider: name,
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
        self.name.clone()
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
mod tests {
    use super::*;

    #[test]
    fn inet_ip() {
        let mut p = ProviderPlane::new();
        p.url = String::from("http://inet-ip.info/ip");
        p.timeout = 2000;
        let addr = p.get_addr().unwrap();
        assert!(addr.addr.is_some());
        assert!(!addr.addr.unwrap().is_private());
    }

    #[test]
    fn httpbin() {
        let mut p = ProviderJson::new();
        p.url = String::from("http://httpbin.org/ip");
        p.key = vec![String::from("origin")];
        p.timeout = 2000;
        let addr = p.get_addr().unwrap();
        assert!(addr.addr.is_some());
        assert!(!addr.addr.unwrap().is_private());
    }

    #[test]
    fn ipify() {
        let mut p = ProviderPlane::new();
        p.url = String::from("http://api.ipify.org");
        p.timeout = 2000;
        let addr = p.get_addr().unwrap();
        assert!(addr.addr.is_some());
        assert!(!addr.addr.unwrap().is_private());
    }

    #[test]
    fn freegeoip() {
        let mut p = ProviderJson::new();
        p.url = String::from("http://freegeoip.net/json");
        p.key = vec![String::from("ip")];
        p.timeout = 2000;
        let addr = p.get_addr().unwrap();
        assert!(addr.addr.is_some());
        assert!(!addr.addr.unwrap().is_private());
    }

    #[test]
    fn toml_load() {
        let _ = ProviderList::from_toml(&DEFAULT_TOML);
    }

    #[test]
    fn provider_any() {
        let mut p0 = ProviderAny::from_toml(&DEFAULT_TOML).unwrap();
        let addr = p0.get_addr().unwrap();
        assert!(addr.addr.is_some());
        assert!(!addr.addr.unwrap().is_private());
    }
}

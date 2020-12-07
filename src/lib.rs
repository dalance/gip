/*!
This crate provides a library for checking global IP address.
This crate get IP address information from IP address checking services like [inet-ip.info](http://inet-ip.info), [ipify.org](http://ipify.org), etc.

# Usage

This crate can be used by adding `gip` to your dependencies in `Cargo.toml`.

```toml
[dependencies]
gip = "0.7.0"
```

# Example

`Provider` trait provide `get_addr` function to check global IP address.
`ProviderDefaultV4` is a `Provider` implementation with built-in providers for IPv4 address.

```rust
use gip::{Provider, ProviderDefaultV4};
let mut p = ProviderDefaultV4::new();
let addr = p.get_addr();
match addr {
    Ok(x) => println!( "Global IPv4 address is {:?}", x.v4addr ),
    Err(_) => (),
}
```

`ProviderDefaultV6` is for IPv6 address.

```rust
use gip::{Provider, ProviderDefaultV6};
let mut p = ProviderDefaultV6::new();
let addr = p.get_addr();
match addr {
    Ok(x) => println!( "Global IPv6 address is {:?}", x.v6addr ),
    Err(_) => (),
}
```

`ProviderDefaultV4` and `ProviderDefaultV6` tries the next provider if a provider is failed to access.
So `get_addr` successes unless all providers failed.

# Built-in providers

`ProviderDefaultV4` and `ProviderDefaultV6` use the built-in provider list ( defined as `DEFAULT_TOML` ):

- [ipv6-test.com](http://ipv6-test.com) ( v4 /v6 )
- [ident.me](http://api.ident.me) ( v4 / v6 )
- [test-ipv6.com](http://test-ipv6.com) ( v4 / v6 )
- [opendns.com](https://www.opendns.com) ( v4 / v6 )
- [akamai.net](https://developer.akamai.com) ( v4 / v6 )

*/

use chrono::{DateTime, Utc};
use core::str::FromStr;
use rand::seq::SliceRandom;
use rand::thread_rng;
use regex::Regex;
use reqwest::blocking::ClientBuilder;
use reqwest::Proxy;
use serde_derive::Deserialize;
use std::io::Read;
use std::net::SocketAddr;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use trust_dns_resolver::config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts};
use trust_dns_resolver::Resolver;

// -------------------------------------------------------------------------------------------------
// Default providers
// -------------------------------------------------------------------------------------------------

/// Built-in providers list
pub static DEFAULT_TOML: &'static str = r#"
    [[providers]]
        name     = "ipv6-test"
        ptype    = "IPv4"
        protocol = "HttpPlane"
        url      = "http://v4.ipv6-test.com/api/myip.php"
        key      = []

    [[providers]]
        name     = "ipv6-test"
        ptype    = "IPv6"
        protocol = "HttpPlane"
        url      = "http://v6.ipv6-test.com/api/myip.php"
        key      = []

    [[providers]]
        name     = "ident.me"
        ptype    = "IPv4"
        protocol = "HttpPlane"
        url      = "http://v4.ident.me/"
        key      = []

    [[providers]]
        name     = "ident.me"
        ptype    = "IPv6"
        protocol = "HttpPlane"
        url      = "http://v6.ident.me/"
        key      = []

    [[providers]]
        name     = "test-ipv6"
        ptype    = "IPv4"
        protocol = "HttpJson"
        url      = "http://ipv4.test-ipv6.com/ip/"
        key      = ["ip"]
        padding  = "callback"

    [[providers]]
        name     = "test-ipv6"
        ptype    = "IPv6"
        protocol = "HttpJson"
        url      = "http://ipv6.test-ipv6.com/ip/"
        key      = ["ip"]
        padding  = "callback"

    [[providers]]
        name     = "opendns.com"
        ptype    = "IPv4"
        protocol = "Dns"
        url      = "myip.opendns.com@resolver1.opendns.com"
        key      = []

    [[providers]]
        name     = "opendns.com"
        ptype    = "IPv6"
        protocol = "Dns"
        url      = "myip.opendns.com@resolver1.opendns.com"
        key      = []

    [[providers]]
        name     = "akamai.net"
        ptype    = "IPv4"
        protocol = "Dns"
        url      = "whoami.akamai.net@ns1-1.akamaitech.net"
        key      = []

    [[providers]]
        name     = "akamai.net"
        ptype    = "IPv6"
        protocol = "Dns"
        url      = "whoami.akamai.net@ns1-1.akamaitech.net"
        key      = []
"#;

// -------------------------------------------------------------------------------------------------
// Error
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    AddrParse(#[from] std::net::AddrParseError),
    #[error(transparent)]
    JsonParse(#[from] serde_json::Error),
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error(transparent)]
    Dns(#[from] trust_dns_resolver::error::ResolveError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("all providers failed to get address")]
    AllProvidersFailed { errors: Vec<Error> },
    #[error("failed to connect ({url})")]
    ConnectionFailed { url: String },
    #[error("failed by timeout to {url} ({timeout}ms)")]
    Timeout { url: String, timeout: usize },
    #[error("failed to parse address ({addr})")]
    AddrParseFailed { addr: String },
    #[error("failed to parse dns string ({url})")]
    DnsParseFailed { url: String },
}

// -------------------------------------------------------------------------------------------------
// GlobalAddress
// -------------------------------------------------------------------------------------------------

/// Global address information
#[derive(Debug)]
pub struct GlobalAddress {
    /// Address checking time
    pub time: DateTime<Utc>,
    /// Access latency
    pub latency: Duration,
    /// Global IP address by IPv4
    pub v4addr: Option<Ipv4Addr>,
    /// Global IP address by IPv6
    pub v6addr: Option<Ipv6Addr>,
    /// Provider name
    pub provider: String,
}

impl GlobalAddress {
    pub fn from_v4(addr: Ipv4Addr, provider: &str, latency: Duration) -> Self {
        GlobalAddress {
            time: Utc::now(),
            latency,
            v4addr: Some(addr),
            v6addr: None,
            provider: String::from(provider),
        }
    }

    pub fn from_v6(addr: Ipv6Addr, provider: &str, latency: Duration) -> Self {
        GlobalAddress {
            time: Utc::now(),
            latency,
            v4addr: None,
            v6addr: Some(addr),
            provider: String::from(provider),
        }
    }
}

// -------------------------------------------------------------------------------------------------
// Provider
// -------------------------------------------------------------------------------------------------

/// Provider describes types that can provide global address information
pub trait Provider {
    /// Get global IP address
    fn get_addr(&mut self) -> Result<GlobalAddress, Error>;
    /// Get provider name
    fn get_name(&self) -> String;
    /// Get provider type
    fn get_type(&self) -> ProviderInfoType;
    /// Set timeout by milliseconds
    fn set_timeout(&mut self, timeout: usize);
    /// Set proxy
    fn set_proxy(&mut self, host: &str, port: u16);
}

// -------------------------------------------------------------------------------------------------
// ProviderInfo
// -------------------------------------------------------------------------------------------------

/// Type of global address from provider
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum ProviderInfoType {
    IPv4,
    IPv6,
}

/// Protocol of provider
#[derive(Debug, Deserialize)]
pub enum ProviderInfoProtocol {
    /// Plane text through HTTP
    HttpPlane,
    /// JSON through HTTP
    HttpJson,
    /// DNS
    Dns,
}

/// Provider information
#[derive(Debug, Deserialize)]
pub struct ProviderInfo {
    /// Provider name
    pub name: String,
    /// Provider type
    pub ptype: ProviderInfoType,
    /// Provider protocol
    pub protocol: ProviderInfoProtocol,
    /// URL for GET
    pub url: String,
    /// Key for JSON format
    pub key: Vec<String>,
    /// Padding for JSON format
    pub padding: Option<String>,
    /// Record for DNS
    pub record: Option<String>,
}

/// Provider information.
///
/// # Examples
/// ```
/// use gip::{ProviderInfo, ProviderInfoProtocol, ProviderInfoType};
/// let p = ProviderInfo::new()
///     .name("inet-ip.info")
///     .ptype(ProviderInfoType::IPv4)
///     .protocol(ProviderInfoProtocol::HttpPlane)
///     .url("http://inet-ip.info/ip")
///     .key(&vec![]);
/// println!("{:?}", p);
/// ```
impl ProviderInfo {
    pub fn new() -> Self {
        ProviderInfo {
            name: String::from(""),
            ptype: ProviderInfoType::IPv4,
            protocol: ProviderInfoProtocol::HttpPlane,
            url: String::from(""),
            key: Vec::new(),
            padding: None,
            record: None,
        }
    }

    pub fn name(self, name: &str) -> Self {
        ProviderInfo {
            name: String::from(name),
            ..self
        }
    }

    pub fn ptype(self, ptype: ProviderInfoType) -> Self {
        ProviderInfo { ptype, ..self }
    }

    pub fn protocol(self, protocol: ProviderInfoProtocol) -> Self {
        ProviderInfo { protocol, ..self }
    }

    pub fn url(self, url: &str) -> Self {
        ProviderInfo {
            url: String::from(url),
            ..self
        }
    }

    pub fn key(self, key: &Vec<String>) -> Self {
        ProviderInfo {
            key: key.clone(),
            ..self
        }
    }

    pub fn padding(self, padding: &str) -> Self {
        ProviderInfo {
            padding: Some(String::from(padding)),
            ..self
        }
    }

    pub fn record(self, record: &str) -> Self {
        ProviderInfo {
            record: Some(String::from(record)),
            ..self
        }
    }

    /// Create `Provider` from this info
    pub fn create(self) -> Box<dyn Provider> {
        match self.protocol {
            ProviderInfoProtocol::HttpPlane => {
                let mut p = Box::new(ProviderHttpPlane::new());
                p.info = self;
                p
            }
            ProviderInfoProtocol::HttpJson => {
                let mut p = Box::new(ProviderHttpJson::new());
                p.info = self;
                p
            }
            ProviderInfoProtocol::Dns => {
                let mut p = Box::new(ProviderDns::new());
                p.info = self;
                p
            }
        }
    }
}

// -------------------------------------------------------------------------------------------------
// ProviderInfoList
// -------------------------------------------------------------------------------------------------

/// Provider information list
#[derive(Debug, Deserialize)]
pub struct ProviderInfoList {
    /// Provider information list
    pub providers: Vec<ProviderInfo>,
}

impl ProviderInfoList {
    /// Load provider info from TOML string
    pub fn from_toml(s: &str) -> Result<ProviderInfoList, Error> {
        let t: ProviderInfoList = toml::from_str(s)?;
        Ok(t)
    }
}

// -------------------------------------------------------------------------------------------------
// ProviderAny
// -------------------------------------------------------------------------------------------------

/// A `Provider` implementation to try multiple providers
pub struct ProviderAny {
    /// Providers for checking global address
    pub providers: Vec<Box<dyn Provider>>,
    /// Provider type
    pub ptype: ProviderInfoType,
}

impl ProviderAny {
    pub fn new() -> Self {
        ProviderAny {
            providers: Vec::new(),
            ptype: ProviderInfoType::IPv4,
        }
    }

    /// Load providers from TOML string
    pub fn from_toml(s: &str) -> Result<Self, Error> {
        let list = ProviderInfoList::from_toml(s)?;
        let mut p = Vec::new();
        for l in list.providers {
            p.push(l.create());
        }

        let ret = ProviderAny {
            providers: p,
            ptype: ProviderInfoType::IPv4,
        };
        Ok(ret)
    }
}

impl Provider for ProviderAny {
    fn get_addr(&mut self) -> Result<GlobalAddress, Error> {
        let mut rng = thread_rng();
        self.providers.shuffle(&mut rng);

        let mut errors = Vec::new();
        for p in &mut self.providers {
            if p.get_type() == self.ptype {
                match p.get_addr() {
                    Ok(ret) => return Ok(ret),
                    Err(err) => errors.push(err),
                }
            }
        }
        Err(Error::AllProvidersFailed { errors })
    }

    fn get_name(&self) -> String {
        String::from("any")
    }

    fn get_type(&self) -> ProviderInfoType {
        self.ptype
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
// ProviderHttpPlane
// -------------------------------------------------------------------------------------------------

/// A `Provider` implementation for checking global address by plane text format.
///
/// # Examples
/// ```
/// use gip::{Provider, ProviderInfo};
/// let mut p = ProviderInfo::new()
///     .url("http://inet-ip.info/ip")
///     .create();
/// let addr = p.get_addr().unwrap();
/// println!( "{:?}", addr.v4addr );
/// ```
pub struct ProviderHttpPlane {
    /// Provider info
    pub info: ProviderInfo,
    /// Timeout
    pub timeout: usize,
    /// Proxy
    pub proxy: Option<(String, u16)>,
}

impl ProviderHttpPlane {
    pub fn new() -> Self {
        ProviderHttpPlane {
            info: ProviderInfo::new(),
            timeout: 1000,
            proxy: None,
        }
    }
}

impl Provider for ProviderHttpPlane {
    fn get_addr(&mut self) -> Result<GlobalAddress, Error> {
        let start = Instant::now();
        let (tx, rx) = mpsc::channel();

        let url = self.info.url.clone();
        let proxy = self.proxy.clone();

        thread::spawn(move || {
            let client = match proxy {
                Some((x, y)) => ClientBuilder::new()
                    .proxy(Proxy::all(&format!("http://{}:{}", x, y)).unwrap())
                    .build()
                    .unwrap(),
                None => ClientBuilder::new().build().unwrap(),
            };
            let res = client.get(&url).send();
            let _ = tx.send(res);
        });

        let mut cnt = 0;
        loop {
            match rx.try_recv() {
                Ok(res) => {
                    let mut res = res.map_err(|_| Error::ConnectionFailed {
                        url: self.info.url.clone(),
                    })?;
                    let mut body = String::new();
                    let _ = res.read_to_string(&mut body);

                    let ret = match self.info.ptype {
                        ProviderInfoType::IPv4 => {
                            let addr = Ipv4Addr::from_str(body.trim())
                                .map_err(|_| Error::AddrParseFailed { addr: body })?;
                            GlobalAddress::from_v4(addr, &self.info.name, start.elapsed())
                        }
                        ProviderInfoType::IPv6 => {
                            let addr = Ipv6Addr::from_str(body.trim())
                                .map_err(|_| Error::AddrParseFailed { addr: body })?;
                            GlobalAddress::from_v6(addr, &self.info.name, start.elapsed())
                        }
                    };

                    return Ok(ret);
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(100));
                    cnt += 1;
                    if cnt > self.timeout / 100 {
                        return Err(Error::Timeout {
                            url: self.info.url.clone(),
                            timeout: self.timeout,
                        });
                    }
                }
            }
        }
    }

    fn get_name(&self) -> String {
        self.info.name.clone()
    }

    fn get_type(&self) -> ProviderInfoType {
        self.info.ptype
    }

    fn set_timeout(&mut self, timeout: usize) {
        self.timeout = timeout
    }

    fn set_proxy(&mut self, host: &str, port: u16) {
        self.proxy = Some((String::from(host), port))
    }
}

// -------------------------------------------------------------------------------------------------
// ProviderHttpJson
// -------------------------------------------------------------------------------------------------

/// A `Provider` implementation for checking global address by JSON format.
///
/// # Examples
/// ```
/// use gip::{ProviderInfo, ProviderInfoProtocol};
/// let mut p = ProviderInfo::new()
///     .protocol(ProviderInfoProtocol::HttpJson)
///     .url("http://ipv4.test-ipv6.com/ip/")
///     .key(&vec![String::from("ip")])
///     .padding("callback")
///     .create();
/// let addr = p.get_addr().unwrap();
/// println!( "{:?}", addr.v4addr );
/// ```
pub struct ProviderHttpJson {
    /// Provider info
    pub info: ProviderInfo,
    /// Timeout
    pub timeout: usize,
    /// Proxy
    pub proxy: Option<(String, u16)>,
}

impl ProviderHttpJson {
    pub fn new() -> Self {
        ProviderHttpJson {
            info: ProviderInfo::new(),
            timeout: 1000,
            proxy: None,
        }
    }
}

impl Provider for ProviderHttpJson {
    fn get_addr(&mut self) -> Result<GlobalAddress, Error> {
        let start = Instant::now();
        let (tx, rx) = mpsc::channel();

        let url = self.info.url.clone();
        let proxy = self.proxy.clone();

        thread::spawn(move || {
            let client = match proxy {
                Some((x, y)) => ClientBuilder::new()
                    .proxy(Proxy::all(&format!("http://{}:{}", x, y)).unwrap())
                    .build()
                    .unwrap(),
                None => ClientBuilder::new().build().unwrap(),
            };
            let res = client.get(&url).send();
            let _ = tx.send(res);
        });

        let mut cnt = 0;
        loop {
            match rx.try_recv() {
                Ok(res) => {
                    let mut res = res.map_err(|_| Error::ConnectionFailed {
                        url: self.info.url.clone(),
                    })?;
                    let mut body = String::new();
                    let _ = res.read_to_string(&mut body);
                    if let Some(ref padding) = self.info.padding {
                        body = {
                            let re = Regex::new(&format!(r"{:}\s*\((.*)\)", padding)).unwrap();
                            let cap = re.captures(&body).unwrap();
                            String::from(cap.get(1).unwrap().as_str())
                        };
                    }
                    let json: serde_json::Value = serde_json::from_str(&body)?;
                    let key = format!("/{}", self.info.key.join("/"));
                    let addr = json.pointer(&key).unwrap().as_str().unwrap();

                    // strip IP address
                    let re = Regex::new(r"([0-9a-zA-Z.:]+)").unwrap();
                    let cap = re.captures(&addr).unwrap();
                    let addr = cap.get(1).unwrap().as_str();

                    let ret = match self.info.ptype {
                        ProviderInfoType::IPv4 => {
                            let addr =
                                Ipv4Addr::from_str(addr).map_err(|_| Error::AddrParseFailed {
                                    addr: String::from(addr),
                                })?;
                            GlobalAddress::from_v4(addr, &self.info.name, start.elapsed())
                        }
                        ProviderInfoType::IPv6 => {
                            let addr =
                                Ipv6Addr::from_str(addr).map_err(|_| Error::AddrParseFailed {
                                    addr: String::from(addr),
                                })?;
                            GlobalAddress::from_v6(addr, &self.info.name, start.elapsed())
                        }
                    };

                    return Ok(ret);
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(100));
                    cnt += 1;
                    if cnt > self.timeout / 100 {
                        return Err(Error::Timeout {
                            url: self.info.url.clone(),
                            timeout: self.timeout,
                        });
                    }
                }
            }
        }
    }

    fn get_name(&self) -> String {
        self.info.name.clone()
    }

    fn get_type(&self) -> ProviderInfoType {
        self.info.ptype
    }

    fn set_timeout(&mut self, timeout: usize) {
        self.timeout = timeout
    }

    fn set_proxy(&mut self, host: &str, port: u16) {
        self.proxy = Some((String::from(host), port))
    }
}

// -------------------------------------------------------------------------------------------------
// ProviderDns
// -------------------------------------------------------------------------------------------------

/// A `Provider` implementation for checking global address through DNS.
/// `url` should be `[request domain name]@[resolver address]`.
///
/// # Examples
/// ```
/// use gip::{Provider, ProviderInfo, ProviderInfoProtocol};
/// let mut p = ProviderInfo::new()
///     .protocol(ProviderInfoProtocol::Dns)
///     .url("myip.opendns.com@resolver1.opendns.com")
///     .create();
/// let addr = p.get_addr().unwrap();
/// println!( "{:?}", addr.v4addr );
/// ```
pub struct ProviderDns {
    /// Provider info
    pub info: ProviderInfo,
    /// Timeout
    pub timeout: usize,
}

impl ProviderDns {
    pub fn new() -> Self {
        ProviderDns {
            info: ProviderInfo::new(),
            timeout: 1000,
        }
    }
}

impl Provider for ProviderDns {
    fn get_addr(&mut self) -> Result<GlobalAddress, Error> {
        let start = Instant::now();
        let mut opts = ResolverOpts::default();
        opts.timeout = Duration::from_millis(self.timeout as u64);

        let resolver = Resolver::new(ResolverConfig::default(), opts)?;

        let (req, srv) = if let Some(x) = self.info.url.find('@') {
            let (req, srv) = self.info.url.split_at(x);
            (req, &srv[1..])
        } else {
            return Err(Error::DnsParseFailed {
                url: self.info.url.clone(),
            });
        };

        let srv = resolver.lookup_ip(srv)?;
        let srv = srv.iter().next().ok_or_else(|| Error::ConnectionFailed {
            url: self.info.url.clone(),
        })?;

        let ns = NameServerConfig {
            socket_addr: SocketAddr::new(srv, 53),
            protocol: Protocol::Udp,
            tls_dns_name: None,
        };
        let mut config = ResolverConfig::new();
        config.add_name_server(ns);
        let resolver = Resolver::new(config, opts)?;

        match self.info.ptype {
            ProviderInfoType::IPv4 => {
                let addr = resolver.ipv4_lookup(req)?;
                let addr = addr.iter().next().ok_or_else(|| Error::ConnectionFailed {
                    url: self.info.url.clone(),
                })?;
                Ok(GlobalAddress::from_v4(
                    *addr,
                    &self.info.name,
                    start.elapsed(),
                ))
            }
            ProviderInfoType::IPv6 => {
                let addr = resolver.ipv6_lookup(req)?;
                let addr = addr.iter().next().ok_or_else(|| Error::ConnectionFailed {
                    url: self.info.url.clone(),
                })?;
                Ok(GlobalAddress::from_v6(
                    *addr,
                    &self.info.name,
                    start.elapsed(),
                ))
            }
        }
    }

    fn get_name(&self) -> String {
        self.info.name.clone()
    }

    fn get_type(&self) -> ProviderInfoType {
        self.info.ptype
    }

    fn set_timeout(&mut self, timeout: usize) {
        self.timeout = timeout
    }

    fn set_proxy(&mut self, _host: &str, _port: u16) {}
}

// -------------------------------------------------------------------------------------------------
// ProviderDefaultV4
// -------------------------------------------------------------------------------------------------

/// A convinient wrapper of `ProviderAny` with default providers for IPv4
///
/// # Examples
/// ```
/// use gip::{Provider, ProviderDefaultV4};
/// let mut p = ProviderDefaultV4::new();
/// let addr = p.get_addr().unwrap();
/// println!( "{:?}", addr.v4addr );
/// ```
pub struct ProviderDefaultV4 {
    provider: ProviderAny,
}

impl ProviderDefaultV4 {
    pub fn new() -> Self {
        ProviderDefaultV4 {
            provider: ProviderAny::from_toml(&DEFAULT_TOML).unwrap(),
        }
    }
}

impl Provider for ProviderDefaultV4 {
    fn get_addr(&mut self) -> Result<GlobalAddress, Error> {
        self.provider.get_addr()
    }

    fn get_name(&self) -> String {
        self.provider.get_name()
    }

    fn get_type(&self) -> ProviderInfoType {
        self.provider.get_type()
    }

    fn set_timeout(&mut self, timeout: usize) {
        self.provider.set_timeout(timeout)
    }

    fn set_proxy(&mut self, host: &str, port: u16) {
        self.provider.set_proxy(host, port)
    }
}

// -------------------------------------------------------------------------------------------------
// ProviderDefaultV6
// -------------------------------------------------------------------------------------------------

/// A convinient wrapper of `ProviderAny` with default providers for IPv6
///
/// # Examples
/// ```
/// use gip::{Provider, ProviderDefaultV6};
/// let mut p = ProviderDefaultV6::new();
/// let addr = p.get_addr();
/// match addr {
///     Ok(x) => println!( "{:?}", x.v6addr ),
///     Err(_) => (),
/// }
/// ```
pub struct ProviderDefaultV6 {
    provider: ProviderAny,
}

impl ProviderDefaultV6 {
    pub fn new() -> Self {
        let mut p = ProviderAny::from_toml(&DEFAULT_TOML).unwrap();
        p.ptype = ProviderInfoType::IPv6;
        ProviderDefaultV6 { provider: p }
    }
}

impl Provider for ProviderDefaultV6 {
    fn get_addr(&mut self) -> Result<GlobalAddress, Error> {
        self.provider.get_addr()
    }

    fn get_name(&self) -> String {
        self.provider.get_name()
    }

    fn get_type(&self) -> ProviderInfoType {
        self.provider.get_type()
    }

    fn set_timeout(&mut self, timeout: usize) {
        self.provider.set_timeout(timeout)
    }

    fn set_proxy(&mut self, host: &str, port: u16) {
        self.provider.set_proxy(host, port)
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
            .name("inet-ip.info")
            .ptype(ProviderInfoType::IPv4)
            .protocol(ProviderInfoProtocol::HttpPlane)
            .url("http://inet-ip.info/ip")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn ipify() {
        let mut p = ProviderInfo::new()
            .name("ipify.org")
            .ptype(ProviderInfoType::IPv4)
            .protocol(ProviderInfoProtocol::HttpPlane)
            .url("http://api.ipify.org")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn ipv6_test() {
        let mut p = ProviderInfo::new()
            .name("ipv6-test.com")
            .ptype(ProviderInfoType::IPv4)
            .protocol(ProviderInfoProtocol::HttpPlane)
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
            .name("ident.me")
            .ptype(ProviderInfoType::IPv4)
            .protocol(ProviderInfoProtocol::HttpPlane)
            .url("http://v4.ident.me")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn test_ipv6() {
        let mut p = ProviderInfo::new()
            .name("test-ipv6.com")
            .ptype(ProviderInfoType::IPv4)
            .protocol(ProviderInfoProtocol::HttpJson)
            .url("http://ipv4.test-ipv6.com/ip/")
            .key(&vec![String::from("ip")])
            .padding("callback")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn test_opendns() {
        let mut p = ProviderInfo::new()
            .name("opendns.com")
            .ptype(ProviderInfoType::IPv4)
            .protocol(ProviderInfoProtocol::Dns)
            .url("myip.opendns.com@resolver1.opendns.com")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn test_akamai() {
        let mut p = ProviderInfo::new()
            .name("akamai.net")
            .ptype(ProviderInfoType::IPv4)
            .protocol(ProviderInfoProtocol::Dns)
            .url("whoami.akamai.net@ns1-1.akamaitech.net")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn toml_load() {
        let _ = ProviderInfoList::from_toml(&DEFAULT_TOML);
    }

    #[test]
    fn provider_any() {
        let mut p = ProviderAny::from_toml(&DEFAULT_TOML).unwrap();
        let addr = p.get_addr().unwrap();
        assert!(addr.v4addr.is_some());
        assert!(!addr.v4addr.unwrap().is_private());
    }

    #[test]
    fn set_proxy() {
        let mut p = ProviderAny::from_toml(&DEFAULT_TOML).unwrap();
        p.set_proxy("example.com", 8080);
    }
}

#[cfg(test)]
mod tests_v6 {
    use super::*;

    #[test]
    fn ipv6_test() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderInfoType::IPv6)
            .protocol(ProviderInfoProtocol::HttpPlane)
            .url("http://v6.ipv6-test.com/api/myip.php")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr();
        match addr {
            Ok(x) => assert!(x.v6addr.is_some()),
            Err(_) => (),
        }
    }

    #[test]
    fn ident_me() {
        let mut p = ProviderInfo::new()
            .ptype(ProviderInfoType::IPv6)
            .protocol(ProviderInfoProtocol::HttpPlane)
            .url("http://v6.ident.me")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr();
        match addr {
            Ok(x) => assert!(x.v6addr.is_some()),
            Err(_) => (),
        }
    }

    #[test]
    fn test_ipv6() {
        let mut p = ProviderInfo::new()
            .name("test-ipv6.com")
            .ptype(ProviderInfoType::IPv6)
            .protocol(ProviderInfoProtocol::HttpJson)
            .url("http://ipv6.test-ipv6.com/ip/")
            .key(&vec![String::from("ip")])
            .padding("callback")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr();
        match addr {
            Ok(x) => assert!(x.v6addr.is_some()),
            Err(_) => (),
        }
    }

    #[test]
    fn test_opendns() {
        let mut p = ProviderInfo::new()
            .name("opendns.com")
            .ptype(ProviderInfoType::IPv6)
            .protocol(ProviderInfoProtocol::Dns)
            .url("myip.opendns.com@resolver1.opendns.com")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr();
        match addr {
            Ok(x) => assert!(x.v6addr.is_some()),
            Err(_) => (),
        }
    }

    #[test]
    fn test_akamai() {
        let mut p = ProviderInfo::new()
            .name("opendns.com")
            .ptype(ProviderInfoType::IPv6)
            .protocol(ProviderInfoProtocol::Dns)
            .url("whoami.akamai.net@ns1-1.akamaitech.net")
            .create();
        p.set_timeout(2000);
        let addr = p.get_addr();
        match addr {
            Ok(x) => assert!(x.v6addr.is_some()),
            Err(_) => (),
        }
    }
}

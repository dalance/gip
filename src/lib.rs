extern crate core;
extern crate hyper;
extern crate rand;
extern crate rustc_serialize;
extern crate time;
extern crate toml;

use core::str::FromStr;
use hyper::Client;
use hyper::header::Connection;
use rustc_serialize::json::Json;
use time::Tm;
use rand::{thread_rng, Rng};
use std::io::Read;
use std::net::Ipv4Addr;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use toml::{Parser, Table, Value};

// ---------------------------------------------------------------------------------------------------------------------
// GlobalAddress
// ---------------------------------------------------------------------------------------------------------------------

pub struct GlobalAddress {
    /// Time of checking address
    pub time: Tm,
    /// Global IP address by IPv4
    pub addr: Option<Ipv4Addr>,
    /// Provider name used for checking address
    pub provider: String,
}

impl GlobalAddress {
    pub fn new() -> Self {
        GlobalAddress {
            time    : time::now(),
            addr    : None,
            provider: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------------------------------------------------

pub trait Provider {
    /// Get global IP address
    fn get_addr   ( &mut self ) -> GlobalAddress;
    /// Get provider name
    fn get_name   ( &self ) -> String;
    /// Set timeout by milliseconds
    fn set_timeout( &mut self, timeout: usize );
}

// ---------------------------------------------------------------------------------------------------------------------
// ProviderInfo
// ---------------------------------------------------------------------------------------------------------------------

/// Format of return value from provider
#[derive(Debug)]
pub enum ProviderType {
    /// Plane text format
    Plane,
    /// JSON format
    Json ,
}

pub struct ProviderInfo {
    /// Provider name
    pub name   : String      ,
    /// Provider format
    pub ptype  : ProviderType,
    /// Timeout
    pub timeout: usize       ,
    /// URL for GET
    pub url    : String      ,
    /// Key for JSON format
    pub key    : Vec<String> ,
}

impl ProviderInfo {
    pub fn new() -> Self {
        ProviderInfo {
            name   : String::from( "" ) ,
            ptype  : ProviderType::Plane,
            timeout: 1000               ,
            url    : String::from( "" ) ,
            key    : Vec::new()         ,
        }
    }

    /// Load provider info from TOML string
    pub fn from_toml( s: &str ) -> Vec<ProviderInfo> {
        let mut ret = Vec::new();
        let mut parser = Parser::new( s );
        match parser.parse() {
            Some( table ) => {
                match table.get( "providers" ) {
                    Some( &Value::Array( ref providers ) ) => {
                        for p in providers {
                            match p {
                                &Value::Table( ref p ) => {
                                    let mut info = ProviderInfo::new();
                                    info.name    = ProviderInfo::get_string ( &p, "name"   , "" );
                                    info.ptype   = ProviderInfo::get_ptype  ( &p, "type"   , ProviderType::Plane );
                                    info.timeout = ProviderInfo::get_usize  ( &p, "timeout", 1000 );
                                    info.url     = ProviderInfo::get_string ( &p, "url"    , "" );
                                    info.key     = ProviderInfo::get_strings( &p, "key" );
                                    ret.push( info );
                                }
                                x => println!( "parse errors: {:?}", x ),
                            }
                        }
                    },
                    None => (),
                    x    => println!( "parse errors: {:?}", x ),
                }
            }
            None => println!( "parse errors: {:?}", parser.errors ),
        }
        ret
    }

    /// Create `Provider` from this info
    pub fn create( &self ) -> Box<Provider> {
        match self.ptype {
            ProviderType::Plane => {
                let mut p = Box::new( ProviderPlane::new() );
                p.name    = self.name.clone();
                p.timeout = self.timeout;
                p.url     = self.url.clone();
                p
            }
            ProviderType::Json => {
                let mut p = Box::new( ProviderJson::new() );
                p.name    = self.name.clone();
                p.timeout = self.timeout;
                p.url     = self.url.clone();
                p.key     = self.key.clone();
                p
            }
        }
    }

    fn get_string( table: &Table, key: &str, default: &str ) -> String {
        let default_val = Value::String( String::from( default ) );
        String::from( table.get( key ).unwrap_or( &default_val ).as_str().unwrap_or( default ) )
    }

    fn get_strings( table: &Table, key: &str ) -> Vec<String> {
        let mut ret = Vec::new();
        let default_val = Value::Array( Vec::new() );
        let default_vec = Vec::new();
        let array = table.get( key ).unwrap_or( &default_val ).as_slice().unwrap_or( &default_vec );
        for a in array {
            match a {
                &Value::String( ref x ) => ret.push( x.clone() ),
                _ => (),
            }
        }
        ret
    }

    fn get_usize( table: &Table, key: &str, default: usize ) -> usize {
        table.get( key ).unwrap_or( &Value::Integer( default as i64 ) ).as_integer().unwrap_or( default as i64 ) as usize
    }

    fn get_ptype( table: &Table, key: &str, default: ProviderType ) -> ProviderType {
        let t = String::from( table.get( key ).unwrap_or( &Value::String( String::new() ) ).as_str().unwrap_or( "" ) );
        match t.as_ref() {
            "Plane" => ProviderType::Plane,
            "Json"  => ProviderType::Json ,
            _       => default            ,
        }
    }

}

// ---------------------------------------------------------------------------------------------------------------------
// ProviderAny
// ---------------------------------------------------------------------------------------------------------------------

/// Provider for checking global address from multiple providers
pub struct ProviderAny {
    /// Providers for checking global address
    pub providers: Vec<Box<Provider>>,
}

impl ProviderAny {
    pub fn new() -> Self {
        ProviderAny {
            providers : Vec::new(),
        }
    }

    /// Load providers from TOML string
    pub fn from_toml( s: &str  ) -> Self {
        let infos = ProviderInfo::from_toml( s );
        let mut p = Vec::new();
        for i in infos {
            p.push( i.create() );
        }

        ProviderAny {
            providers : p,
        }
    }
}

impl Provider for ProviderAny {
    fn get_addr( &mut self ) -> GlobalAddress {
        let mut rng = thread_rng();
        rng.shuffle( &mut self.providers );

        for p in &mut self.providers {
            let ret = p.get_addr();
            if ret.addr.is_some() {
                return ret
            }
        }
        return GlobalAddress::new();
    }

    fn get_name( &self ) -> String {
        String::from( "any" )
    }

    fn set_timeout( &mut self, timeout: usize ) {
        for p in &mut self.providers {
            p.set_timeout( timeout )
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// ProviderPlane
// ---------------------------------------------------------------------------------------------------------------------

/// Provider for checking global address by plane text format.
///
/// # Examples
/// ```
/// use gip::{Provider, ProviderPlane};
/// let mut p = ProviderPlane::new();
/// p.url = String::from( "http://inet-ip.info/ip" );
/// let addr = p.get_addr();
/// println!( "{:?}", addr.addr );
/// ```
pub struct ProviderPlane {
    /// Provider name
    pub name   : String,
    /// URL for GET
    pub url    : String,
    /// Timeout
    pub timeout: usize ,
}

impl ProviderPlane {
    pub fn new() -> Self {
        ProviderPlane {
            name   : String::new(),
            url    : String::new(),
            timeout: 1000,
        }
    }
}

impl Provider for ProviderPlane {
    fn get_addr( &mut self ) -> GlobalAddress {
        let ( tx, rx ) = mpsc::channel();

        let name = self.name.clone();
        let url  = self.url.clone();
        thread::spawn( move || {
            let client = Client::new();
            let res = client.get( &url ).header( Connection::close() ).send();

            let mut body = String::new();
            match res {
                Ok ( mut x ) => { let _ = x.read_to_string( &mut body ); },
                Err( _     ) => return,
            }

            let addr = match Ipv4Addr::from_str( body.trim() ) {
                Ok ( x ) => Some( x ),
                Err( _ ) => return,
            };

            let ret = GlobalAddress {
                time    : time::now(),
                addr    : addr,
                provider: name,
            };
            let _ = tx.send( ret );
        } );

        let mut cnt = 0;
        loop {
            match rx.try_recv() {
                Ok ( x ) => return x,
                Err( _ ) => {
                    thread::sleep( Duration::from_millis( 100 ) );
                    cnt += 1;
                    if cnt > self.timeout / 100 {
                        return GlobalAddress::new();
                    }
                },
            }
        }
    }

    fn get_name( &self ) -> String {
        self.name.clone()
    }

    fn set_timeout( &mut self, timeout: usize ) {
        self.timeout = timeout
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// ProviderJson
// ---------------------------------------------------------------------------------------------------------------------

/// Provider for checking global address by JSON format.
///
/// # Examples
/// ```
/// use gip::{Provider, ProviderJson};
/// let mut p = ProviderJson::new();
/// p.url = String::from( "http://httpbin.org/ip" );
/// p.key = vec!["origin".to_string()];
/// let addr = p.get_addr();
/// println!( "{:?}", addr.addr );
/// ```
pub struct ProviderJson {
    /// Provider name
    pub name   : String,
    /// URL for GET
    pub url    : String,
    /// Key for JSON format
    pub key    : Vec<String>,
    /// Timeout
    pub timeout: usize ,
}

impl ProviderJson {
    pub fn new() -> Self {
        ProviderJson {
            name   : String::new(),
            url    : String::new(),
            key    : Vec::new(),
            timeout: 1000,
        }
    }
}

impl Provider for ProviderJson {
    fn get_addr( &mut self ) -> GlobalAddress {
        let ( tx, rx ) = mpsc::channel();

        let name = self.name.clone();
        let url  = self.url.clone();
        let key  = self.key.clone();
        thread::spawn( move || {
            let client = Client::new();
            let res = client.get( &url ).header( Connection::close() ).send();

            let mut body = String::new();
            match res {
                Ok ( mut x ) => { let _ = x.read_to_string( &mut body ); },
                Err( _     ) => return,
            }

            let json = match Json::from_str( &body ) {
                Ok ( x ) => x,
                Err( _ ) => return,
            };

            let key: Vec<&str> = key.iter().map(|x| { let r: &str = &x; r } ).collect();
            let addr = match json.find_path( &key[..] ) {
                Some( &Json::String( ref x ) ) => x,
                Some( _                      ) => return,
                None                           => return,
            };

            let addr = match Ipv4Addr::from_str( &addr ) {
                Ok ( x ) => Some( x ),
                Err( _ ) => return,
            };

            let ret = GlobalAddress {
                time    : time::now(),
                addr    : addr,
                provider: name,
            };
            let _ = tx.send( ret );
        } );

        let mut cnt = 0;
        loop {
            match rx.try_recv() {
                Ok ( x ) => return x,
                Err( _ ) => {
                    thread::sleep( Duration::from_millis( 100 ) );
                    cnt += 1;
                    if cnt > self.timeout / 100 {
                        return GlobalAddress::new();
                    }
                },
            }
        }
    }

    fn get_name( &self ) -> String {
        self.name.clone()
    }

    fn set_timeout( &mut self, timeout: usize ) {
        self.timeout = timeout
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    static DEFAULT_TOML: &'static str = r#"
        [[providers]]
            name    = "inet-ip.info"
            ptype   = "Plane"
            timeout = 1000
            url     = "http://inet-ip.info/ip"

        [[providers]]
            name    = "myexternalip.com"
            ptype   = "Plane"
            timeout = 1000
            url     = "http://myexternalip.com/raw"

        [[providers]]
            name    = "globalip.me"
            ptype   = "Plane"
            timeout = 1000
            url     = "http://globalip.me?ip"

        [[providers]]
            name    = "ipify.org"
            ptype   = "Plane"
            timeout = 1000
            url     = "http://api.ipify.org"

        [[providers]]
            name    = "httpbin.org"
            ptype   = "Json"
            timeout = 1000
            url     = "http://httpbin.org/ip"
            key     = ["origin"]
    "#;

    #[test]
    fn inet_ip() {
        let mut p = ProviderPlane::new();
        p.url = String::from( "http://inet-ip.info/ip" );
        let addr = p.get_addr();
        println!( "{:?}", addr.addr );
    }

    #[test]
    fn httpbin() {
        let mut p = ProviderJson::new();
        p.url = String::from( "http://httpbin.org/ip" );
        p.key = vec!["origin".to_string()];
        let addr = p.get_addr();
        println!( "{:?}", addr.addr );
    }

    #[test]
    fn myexternalip() {
        let mut p = ProviderJson::new();
        p.url = String::from( "http://myexternalip/raw" );
        p.key = vec!["origin".to_string()];
        let addr = p.get_addr();
        println!( "{:?}", addr.addr );
    }

    #[test]
    fn ipify() {
        let mut p = ProviderPlane::new();
        p.url = String::from( "http://api.ipify.org" );
        let addr = p.get_addr();
        println!( "{:?}", addr.addr );
    }

    #[test]
    fn globalip() {
        let mut p = ProviderPlane::new();
        p.url = String::from( "http://globalip.me?ip" );
        let addr = p.get_addr();
        println!( "{:?}", addr.addr );
    }

    #[test]
    fn provider_info() {
        let mut info = ProviderInfo::new();
        info.name    = String::from( "test" );
        info.ptype   = ProviderType::Json;
        info.timeout = 100;
        info.url     = String::from( "http://httpbin.org/ip" );
        info.key     = vec!["origin".to_string()];

        let mut p = info.create();
        let addr = p.get_addr();
        println!( "{:?}", addr.addr );
    }

    #[test]
    fn toml_load() {
        let _ = ProviderInfo::from_toml( &DEFAULT_TOML );
    }

    #[test]
    fn provider_any() {
        let mut p0 = ProviderAny::from_toml( &DEFAULT_TOML );
        let addr = p0.get_addr();
        println!( "{:?}", addr.addr );
    }
}


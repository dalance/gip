extern crate clap;
#[macro_use]
extern crate error_chain;
extern crate gip;
#[macro_use]
extern crate structopt;

use gip::{Provider, ProviderAny};
use std::env::home_dir;
use std::fs::File;
use std::io::Read;
use structopt::StructOpt;

// -------------------------------------------------------------------------------------------------
// Usage
// -------------------------------------------------------------------------------------------------

#[derive(Debug, StructOpt)]
#[structopt(name = "gip")]
#[structopt(raw(long_version = "option_env!(\"LONG_VERSION\").unwrap_or(env!(\"CARGO_PKG_VERSION\"))"))]
#[structopt(raw(setting = "clap::AppSettings::ColoredHelp"))]
pub struct Opt {
    /// Show by plane text ( default )
    #[structopt(short = "p", long = "plane")]
    pub show_plane: bool,

    /// Show by plane text without line break
    #[structopt(short = "s", long = "string")]
    pub show_string: bool,

    /// Show by JSON
    #[structopt(short = "j", long = "json")]
    pub show_json: bool,

    /// Timeout per each provider by milliseconds
    #[structopt(long = "timeout", default_value = "1000")]
    pub timeout: usize,

    /// Key string of JSON format
    #[structopt(long = "json-key", default_value = "ip")]
    pub json_key: String,

    /// Proxy for HTTP access ( "host:port" )
    #[structopt(long = "proxy")]
    pub proxy: Option<String>,

    /// Show provider list
    #[structopt(short = "l", long = "list")]
    pub show_list: bool,

    /// Show verbose message
    #[structopt(short = "V", long = "verbose")]
    pub verbose: bool,
}

// -------------------------------------------------------------------------------------------------
// Error
// -------------------------------------------------------------------------------------------------

error_chain! {
    links {
        Gip(::gip::Error, ::gip::ErrorKind);
    }
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

quick_main!(run);

pub fn run() -> Result<()> {
    let opt = Opt::from_args();
    run_opt(&opt)
}

pub fn run_opt(opt: &Opt) -> Result<()> {

    let giprc = match home_dir() {
        Some(p) => {
            let mut p = p.clone();
            p.push(".gip.toml");
            if p.exists() {
                Some(p)
            } else {
                None
            }
        }
        None => None,
    };

    let mut client = match giprc {
        Some(p) => {
            let mut f = File::open(p).unwrap();
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            ProviderAny::from_toml(&s).unwrap()
        }
        None => ProviderAny::from_toml(&gip::DEFAULT_TOML).unwrap(),
    };

    if opt.show_list {
        for p in &client.providers {
            println!("{}", p.get_name());
        }
        return Ok(());
    }

    client.set_timeout(opt.timeout);

    if opt.proxy.is_some() {
        let proxy_str = opt.proxy.clone().unwrap();
        let (host, port) = proxy_str.split_at(proxy_str.find(':').unwrap_or(0));
        let port = port.trim_matches(':').parse::<u16>();
        match port {
            Ok(p) => client.set_proxy(host, p),
            Err(_) => println!(
                "Proxy format error: {} ( must be \"host:port\" format )",
                proxy_str
            ),
        }
    }

    let addr = client.get_addr().unwrap();
    let addr_str = match addr.addr {
        Some(x) => format!("{:?}", x),
        None => format!("Failed"),
    };

    if opt.verbose {
        println!("IP Address: {}", addr_str);
        println!("Provider  : {}", addr.provider);
        println!("Check Time: {}", addr.time.rfc822());
    } else {
        if opt.show_string {
            print!("{}", addr_str);
        } else if opt.show_json {
            println!("{{\"{}\": \"{}\"}}", opt.json_key, addr_str);
        } else {
            println!("{}", addr_str);
        }
    }

    return Ok(());
}

// -------------------------------------------------------------------------------------------------
// Test
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run() {
        let args = vec!["gip"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_verbose() {
        let args = vec!["gip", "-V"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_string() {
        let args = vec!["gip", "-s"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_json() {
        let args = vec!["gip", "-j"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }

    #[test]
    fn test_list() {
        let args = vec!["gip", "-l"];
        let opt = Opt::from_iter(args.iter());
        let ret = run_opt(&opt);
        assert!(ret.is_ok());
    }
}


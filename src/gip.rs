use dirs::home_dir;
use error_chain::{error_chain, quick_main};
use gip::{Provider, ProviderAny, ProviderInfoType};
use std::fs::File;
use std::io::Read;
use structopt::{clap, StructOpt};

// -------------------------------------------------------------------------------------------------
// Usage
// -------------------------------------------------------------------------------------------------

#[derive(Debug, StructOpt)]
#[structopt(name = "gip")]
#[structopt(
    long_version(option_env!("LONG_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")))
)]
#[structopt(setting(clap::AppSettings::ColoredHelp))]
#[structopt(setting(clap::AppSettings::DeriveDisplayOrder))]
pub struct Opt {
    /// IPv4 address ( default )
    #[structopt(short = "4", long = "v4", conflicts_with = "v6")]
    pub v4: bool,

    /// IPv6 address
    #[structopt(short = "6", long = "v6", conflicts_with = "v4")]
    pub v6: bool,

    /// Show by plane text ( default )
    #[structopt(
        short = "p",
        long = "plane",
        conflicts_with = "show_string",
        conflicts_with = "show_json"
    )]
    pub show_plane: bool,

    /// Show by plane text without line break
    #[structopt(
        short = "s",
        long = "string",
        conflicts_with = "show_plane",
        conflicts_with = "show_json"
    )]
    pub show_string: bool,

    /// Show by JSON
    #[structopt(
        short = "j",
        long = "json",
        conflicts_with = "show_plane",
        conflicts_with = "show_string"
    )]
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
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,
}

// -------------------------------------------------------------------------------------------------
// Error
// -------------------------------------------------------------------------------------------------

error_chain! {
    links {
        Gip(::gip::Error, ::gip::ErrorKind);
    }
    foreign_links {
        Io(::std::io::Error);
        ParseInt(::std::num::ParseIntError);
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
        Some(mut p) => {
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
            let mut f =
                File::open(&p).chain_err(|| format!("failed to open {}", p.to_string_lossy()))?;
            let mut s = String::new();
            let _ = f.read_to_string(&mut s);
            ProviderAny::from_toml(&s)?
        }
        None => ProviderAny::from_toml(&gip::DEFAULT_TOML)?,
    };

    if opt.v6 {
        client.ptype = ProviderInfoType::IPv6;
    }

    if opt.show_list {
        for p in &client.providers {
            println!("{:?}: {}", p.get_type(), p.get_name());
        }
        return Ok(());
    }

    client.set_timeout(opt.timeout);

    if opt.proxy.is_some() {
        let proxy_str = opt.proxy.clone().unwrap();
        let (host, port) = proxy_str.split_at(proxy_str.find(':').unwrap_or(0));
        let port = port
            .trim_matches(':')
            .parse::<u16>()
            .chain_err(|| format!("failed to parse proxy: {}", proxy_str))?;
        client.set_proxy(host, port);
    }

    let addr = client.get_addr()?;
    let addr_str = if opt.v6 {
        format!("{:?}", addr.v6addr.unwrap())
    } else {
        format!("{:?}", addr.v4addr.unwrap())
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
        let args = vec!["gip", "-v"];
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

    #[test]
    fn test_v6() {
        let args = vec!["gip", "-6"];
        let opt = Opt::from_iter(args.iter());
        let _ = run_opt(&opt);
    }

    #[test]
    fn test_proxy() {
        let args = vec!["gip", "--proxy", "example.com:8080"];
        let opt = Opt::from_iter(args.iter());
        let _ = run_opt(&opt);
    }
}

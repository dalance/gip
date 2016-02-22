extern crate docopt;
extern crate gip;
extern crate rustc_serialize;

use docopt::Docopt;
use gip::{Provider, ProviderAny};
use std::env::home_dir;
use std::fs::File;
use std::io::Read;

// ---------------------------------------------------------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------------------------------------------------------

static USAGE: &'static str = "
Show global ip address

Usage:
    gip [options]

Options:
    -p --plane        Show by plane text ( default )
    -s --string       Show by plane text without line break
    -j --json         Show by JSON

    --timeout <ms>    timeout per each provider by milliseconds [default: 1000]
    --json-key <key>  Key string of JSON format [default: ip]

    -l --list         Show provider list
    -h --help         Show this message
    -V --verbose      Show verbose message
    -v --version      Show version
";

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
        url     = "https://api.ipify.org"

    [[providers]]
        name    = "httpbin.org"
        ptype   = "Json"
        timeout = 1000
        url     = "http://httpbin.org/ip"
        key     = ["origin"]
"#;

static VERSION: &'static str = env!( "CARGO_PKG_VERSION" );
static BUILD_TIME  : Option<&'static str> = option_env!( "BUILD_TIME"   );
static GIT_REVISION: Option<&'static str> = option_env!( "GIT_REVISION" );

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_plane   : bool ,
    flag_string  : bool ,
    flag_json    : bool ,
    flag_timeout : usize,
    flag_json_key: String,
    flag_list    : bool ,
    flag_verbose : bool ,
}

// ---------------------------------------------------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------------------------------------------------

fn main() {
    let version = if BUILD_TIME.is_some() {
        format!( "gip version {} ( {} {} )", VERSION, GIT_REVISION.unwrap_or( "" ), BUILD_TIME.unwrap() )
    } else {
        format!( "gip version {}", VERSION )
    };

    let args: Args = Docopt::new( USAGE ).and_then( |d| d.version( Some( version ) ).decode() ).unwrap_or_else( |e| e.exit() );

    let giprc = match home_dir() {
        Some( p ) => {
            let mut p = p.clone();
            p.push( ".gip.toml" );
            if p.exists() {
                Some( p )
            } else {
                None
            }
        },
        None => None,
    };

    let mut client = match giprc {
        Some( p ) => {
            let mut f = File::open( p ).unwrap();
            let mut s = String::new();
            let _ = f.read_to_string( &mut s );
            ProviderAny::from_toml( &s )
        },
        None => ProviderAny::from_toml( &DEFAULT_TOML )
    };

    if args.flag_list {
        for p in &client.providers {
            println!( "{}", p.get_name() );
        }
        return
    }

    let addr = client.get_addr();
    let addr_str = match addr.addr {
        Some( x ) => format!( "{:?}", x ),
        None      => format!( "Failed" ),
    };

    if args.flag_verbose {
        println!( "IP Address: {}", addr_str  );
        println!( "Provider  : {}", addr.provider );
        println!( "Check Time: {}", addr.time.rfc822() );
    } else {
        if args.flag_string {
            print!( "{}", addr_str );
        } else if args.flag_json {
            println!( "{{\"{}\": \"{}\"}}", args.flag_json_key, addr_str );
        } else {
            println!( "{}", addr_str );
        }
    }
}

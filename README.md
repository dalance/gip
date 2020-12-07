# gip

[![Actions Status](https://github.com/dalance/gip/workflows/Regression/badge.svg)](https://github.com/dalance/gip/actions)
[![Crates.io](https://img.shields.io/crates/v/gip.svg)](https://crates.io/crates/gip)
[![Docs.rs](https://docs.rs/gip/badge.svg)](https://docs.rs/gip)
[![codecov](https://codecov.io/gh/dalance/gip/branch/master/graph/badge.svg)](https://codecov.io/gh/dalance/gip)

**gip** is a command-line tool and Rust library to check global IP address.

## Install
Download from [release page](https://github.com/dalance/gip/releases/latest), and extract to the directory in PATH.

Alternatively you can install by [cargo](https://crates.io).

```
cargo install gip
```

## Usage

```
gip 0.3.2-pre
dalance <dalance@gmail.com>
A library and command-line frontend to check global IP address

USAGE:
    gip [FLAGS] [OPTIONS]

FLAGS:
    -4, --v4         IPv4 address ( default )
    -6, --v6         IPv6 address
    -p, --plane      Show by plane text ( default )
    -s, --string     Show by plane text without line break
    -j, --json       Show by JSON
    -l, --list       Show provider list
    -v, --verbose    Show verbose message
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --timeout <timeout>      Timeout per each provider by milliseconds [default: 1000]
        --json-key <json_key>    Key string of JSON format [default: ip]
        --proxy <proxy>          Proxy for HTTP access ( "host:port" )
```

## Providers
Currently built-in service providers are the followings.

- [ipv6-test.com](http://ipv6-test.com) ( v4 /v6 )
- [ident.me](http://api.ident.me) ( v4 / v6 )
- [test-ipv6.com](http://test-ipv6.com) ( v4 / v6 )
- [opendns.com](https://www.opendns.com) ( v4 / v6 )
- [akamai.net](https://developer.akamai.com) ( v4 / v6 )

If you want to change providers, providers can be set by `$HOME/.gip.toml` like the following.

```
[[providers]]
    name     = "ident.me"
    ptype    = "IPv4"
    protocol = "HttpPlane"
    url      = "http://v4.ident.me/"
    key      = []

[[providers]]
    name     = "test-ipv6"
    ptype    = "IPv4"
    protocol = "HttpJson"
    url      = "http://ipv4.test-ipv6.com/ip/"
    key      = ["ip"]
    padding  = "callback"

[[providers]]
    name     = "opendns.com"
    ptype    = "IPv4"
    protocol = "Dns"
    url      = "myip.opendns.com@resolver1.opendns.com"
    key      = []
```

## Library

**gip** is provided as Rust library.

```Cargo.toml
gip = "0.7.0"
```

[Documentation](https://docs.rs/gip)

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

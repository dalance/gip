# gip

[![Build Status](https://travis-ci.org/dalance/gip.svg?branch=master)](https://travis-ci.org/dalance/gip)
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
gip                    // show global IP address by plane text.
gip -s                 // show global IP address by plane text without line break.
gip -j                 // show global IP address by JSON.                        ( ex. {"ip", "xxx.xxx.xxx.xxx"} )
gip -j --json-key key  // show global IP address by JSON with the specified key. ( ex. {"key", "xxx.xxx.xxx.xxx"} )
```

## Providers
Currently built-in service providers are the followings.

- [inet-ip.info](http://inet-ip.info)
- [ipify.org](http://ipify.org)
- [httpbin.org](http://httpbin.org)
- [freegeoip.net](http://freegeoip.net)

If you want to change providers, providers can be set by `$HOME/.gip.toml` like the following.

```
[[providers]]
    name    = "inet-ip.info"
    ptype   = "Plane"
    timeout = 1000
    url     = "http://inet-ip.info/ip"

[[providers]]
    name    = "httpbin.org"
    ptype   = "Json"
    timeout = 1000
    url     = "http://httpbin.org/ip"
    key     = ["origin"]
```

## Library

**gip** is provided as Rust library.

```Cargo.toml
gip = "*"
```

[Documentation](https://docs.rs/gip)

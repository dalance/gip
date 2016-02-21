# gip

**gip** is a command-line tool to get global IP address.

## Install
Download from [release page](https://github.com/dalance/gip/releases/latest), and extract to the directory in PATH.

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
- [myexternalip.com](http://myexternalip.com)
- [globalip.me](http://globalip.me)
- [ipify.org](http://ipify.org)
- [httpbin.org](http://httpbin.org)

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

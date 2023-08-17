# socks5ws

app for running SOCKS5 server as windows service

currently used:

- [fast-socks5](https://github.com/dizda/fast-socks5) - socks5 server lib
- [windows-service](https://github.com/mullvad/windows-service-rs) - windows service lib

## Usage

use subcommands:

- `install`
- `uninstall`
- `start`
- `stop`

and:

- `save-config` - save default config to a file in .exe folder
- `serve` - run server as foreground proccess

don't use `run`, this is for windows, as service entrypoint

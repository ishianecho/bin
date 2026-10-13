# Proxy

Proxy URLs are stored in `~/.config/proxy/config` and applied through
environment variables only: `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`,
`NO_PROXY`. No per-tool configuration is written. SSH uses the managed
`proxy ssh` helper.

## Commands

```sh
proxy set http://127.0.0.1:10808 socks5://127.0.0.1:10808
proxy set --http http://127.0.0.1:10808 --socks socks5://127.0.0.1:10808 \
  --no-proxy "localhost,127.0.0.1,::1,.internal"
proxy show
proxy sync
proxy clear
```

Windows: `proxy.exe` with the same subcommands.

Passing only `--http` (or only `--socks`) to `proxy set` merges with the
saved configuration; `proxy clear` removes everything.

`proxy ssh HOST PORT` relays stdin/stdout through the configured SOCKS or
HTTP proxy (no `nc`/`ncat` needed) and is meant for `ProxyCommand`:

```
ProxyCommand sh -c 'exec "$HOME/.local/bin/proxy" ssh %h %p'
```

When the socks port managed by `xrayctl` changes, `xrayctl port set`
calls `proxy set` automatically (see [`XRAY`](XRAY.md)).

Transparent proxying (see [`XRAY`](XRAY.md)) covers tools that ignore the
environment.

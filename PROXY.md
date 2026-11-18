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
proxy off -- curl https://example.com
proxy on -- git clone https://github.com/example/repo
```

`proxy off COMMAND` runs COMMAND with every proxy environment variable
removed; `proxy on COMMAND` runs COMMAND with the saved configuration applied
to its environment even when the current shell has no proxy variables set.

Windows: `proxy.exe` with the same subcommands.

Passing only `--http` (or only `--socks`) to `proxy set` merges with the
saved configuration; `proxy clear` removes everything.

`proxy ssh HOST PORT` relays stdin/stdout through the configured SOCKS or
HTTP proxy (no `nc`/`ncat` needed) and is meant for `ProxyCommand`:

```
ProxyCommand sh -c 'exec "$HOME/.local/bin/proxy" ssh %h %p'
```

When the socks port managed by `xrayctl` or `sbctl` changes, their
`port set` calls `proxy set` automatically (see [`XRAY`](XRAY.md),
[`SBCTL`](SBCTL.md)).

Transparent proxying (see [`XRAY`](XRAY.md), [`SBCTL`](SBCTL.md)) covers
tools that ignore the environment.

## Scenarios

Match the setup to what the machine already has:

- A company proxy client is already installed (Clash Verge, v2rayN, any
  GUI app): point `proxy` at its local port (Clash Verge mixed port
  defaults to 7890):
  `proxy set --http http://127.0.0.1:7890 --socks socks5://127.0.0.1:7890`.
- Only a corporate proxy URL is provided:
  `proxy set --http http://user:pass@proxy.corp.example:8080 --no-proxy "localhost,127.0.0.1,.internal"`
  so intranet hosts stay direct.
- Nothing is provided: run your own node with `xrayctl` (tproxy,
  Linux/Windows) or `sbctl` (sing-box tun, Linux only); both keep
  `proxy` pointing at the local port automatically.

macOS has no tproxy: `proxy` and `xrayctl` (socks/mixed inbound only,
no service management) work; `sbctl` is Linux-only. A company client
plus `proxy set` is the usual macOS setup.

# bin

Personal CLI tools. `proxy`, `xrayctl`, and `sbctl` are Rust binaries; `ghf`
and `updnvim` are shell scripts.

## Tools

### ghf

Interactive GitHub browser built on `gh`, `fzf`, and `jq`. Covers notifications,
repos, issues, pull requests, and Actions runs.

```
ghf              main menu
ghf notif        notifications
ghf repo         repositories
ghf issue [repo] issues
ghf pr [repo]    pull requests
ghf action [repo] Actions runs
```

### updnvim

Downloads and installs Neovim (nightly by default, override with
`LATEST_RELEASE=v0.11.0`) into the current nvim prefix, or
`~/.local/opt/nvim` when nvim is not installed yet. On Windows use
`updnvim.ps1`.

### xrayctl

State-driven manager for a local Xray service (vless + reality): node and
routing-rule management, port configuration, config generation with
`xray run -test` validation, service restarts (systemd on Linux, NSSM on
Windows), and Linux transparent-proxy rules. See [`XRAY.md`](XRAY.md).

### sbctl

State-driven manager for a local sing-box service (tun + vless + reality,
Linux only): node and routing-rule management, optional wireguard mesh
carried inside the tunnel, config generation with `sing-box check`
validation, and systemd service restarts. Replaces `xrayctl`; see
[`SBCTL.md`](SBCTL.md).

### proxy

Manages a proxy configuration in `~/.config/proxy/config` and applies it
through environment variables. Shell sessions and `ssh` pick the file up
directly; no per-tool configuration is written.

```
proxy set --http http://127.0.0.1:10808 --socks socks5://127.0.0.1:10808
proxy set --http http://127.0.0.1:10808
proxy show
proxy sync
proxy clear
proxy ssh HOST PORT
```

`proxy ssh` is meant for `ProxyCommand` in `~/.ssh/config`:

```
ProxyCommand sh -c 'exec "$HOME/.local/bin/proxy" ssh %h %p'
```

On Windows `proxy set` and `proxy sync` also update user-level environment
variables. See [`PROXY.md`](PROXY.md).

### notify

Sends a desktop notification on macOS (`osascript`) and Linux
(`notify-send`), with an optional title and system sound
(`canberra-gtk-play` on Linux, when available).

```
notify "Build complete"
notify "Task done" --title "My App"
notify "Hello" --sound Glass
```

### port

Inspects what is listening on a TCP port, or kills the processes holding
it (via `lsof`):

```
port 8080
port clear 8080
```

## Install

Each release publishes static binaries for linux, darwin, and windows
(amd64 and arm64): `proxy-<os>-<arch>`, `xrayctl-<os>-<arch>[.exe]`, and
`sbctl-<os>-<arch>[.exe]` at `releases/latest/download`. The shell scripts
(`ghf`, `updnvim`, `updnvim.ps1`) can be taken from the repository
directly. Each tool implements a `completion <shell>` subcommand (zsh,
bash, fish, and powershell for the Rust tools; zsh and bash for ghf);
write the output to a file on your completion path, for example
`~/.config/zfunc/_proxy` (zsh) or `~/.config/bash/completions/proxy.bash`
(bash).

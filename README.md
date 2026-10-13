# bin

Personal CLI tools, released per platform from this repository.

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
`~/.local/opt/nvim` when nvim is not installed yet.

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

On Windows the same commands are available via `proxy.cmd` / `proxy.ps1`;
`proxy sync` and `proxy set` also update user-level environment variables.

### completion.sh

Emits shell completion scripts:

```
completion.sh zsh ghf
completion.sh zsh proxy
completion.sh bash ghf
completion.sh bash proxy
```

Write the output to a file on your completion path, for example
`~/.config/zfunc/_proxy` (zsh) or `~/.config/bash/completions/proxy.bash` (bash).

## Install

Each release publishes two archives: `bin-posix.tar.gz` (extract into
`~/.local/bin`) and `bin-win.zip` (extract into `%USERPROFILE%\.local\bin`).

## Development

Tagging `v*` triggers the release workflow, which builds both archives and
attaches them to a GitHub release.

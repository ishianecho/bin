#!/usr/bin/env bash

set -euo pipefail

usage() {
  echo "usage: completion.sh <zsh|bash> <ghf|proxy>" >&2
}

zsh_ghf() {
  cat << 'EOF'
#compdef ghf

_ghf() {
  local -a commands
  commands=(
    'n:notifications'
    'notif:notifications'
    'notifications:notifications'
    'r:repositories'
    'repo:repositories'
    'repos:repositories'
    'repositories:repositories'
    'i:issues'
    'issue:issues'
    'issues:issues'
    'p:pull requests'
    'pr:pull requests'
    'prs:pull requests'
    'pulls:pull requests'
    'a:actions'
    'action:actions'
    'actions:actions'
    'menu:open the main menu'
    'help:show help'
    '--help:show help'
    '-h:show help'
  )

  if (( CURRENT == 2 )); then
    _describe 'ghf command' commands
    return
  fi

  case "${words[2]}" in
    i|issue|issues|p|pr|prs|pulls|a|action|actions)
      _message 'repository (owner/name)'
      ;;
  esac
}

compdef _ghf ghf
EOF
}

zsh_proxy() {
  cat << 'EOF'
#compdef proxy

_proxy() {
  local -a commands
  commands=(
    'set:save and apply a proxy configuration'
    'ssh:connect through the configured proxy'
    'clear:remove all proxy settings'
    'show:show the saved proxy configuration'
    'sync:re-apply the saved configuration to the environment'
    'help:show help'
  )

  if (( CURRENT == 2 )); then
    _describe 'proxy command' commands
    return
  fi

  case "${words[2]}" in
    set)
      _arguments \
        '--http[HTTP/HTTPS proxy url]:url' \
        '--socks[SOCKS proxy url]:url' \
        '--no-proxy[comma-separated hosts which bypass the proxy]:list' \
        '(-h --help)'{-h,--help}'[show help]' \
        ':http proxy url' \
        ':socks proxy url'
      ;;
    ssh)
      _message 'host'
      ;;
  esac
}

compdef _proxy proxy
EOF
}

bash_ghf() {
  cat << 'EOF'
_ghf() {
    local cur
    cur="${COMP_WORDS[COMP_CWORD]}"
    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=($(compgen -W "n notif notifications r repo repos repositories i issue issues p pr prs pulls a action actions menu help --help -h" -- "$cur"))
    fi
}
complete -F _ghf ghf
EOF
}

bash_proxy() {
  cat << 'EOF'
_proxy() {
    local cur prev
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    if [ "$COMP_CWORD" -eq 1 ]; then
        COMPREPLY=($(compgen -W "set ssh clear show sync help" -- "$cur"))
        return 0
    fi

    case "$prev" in
        --http | --socks | --no-proxy)
            return 0
            ;;
    esac

    case "${COMP_WORDS[1]}" in
        set)
            COMPREPLY=($(compgen -W "--http --socks --no-proxy -h --help" -- "$cur"))
            ;;
    esac
}

complete -F _proxy proxy
EOF
}

case "${1:-}" in
  zsh | bash) ;;
  *)
    usage
    exit 2
    ;;
esac

case "${2:-}" in
  ghf | proxy) ;;
  *)
    usage
    exit 2
    ;;
esac

"${1}_${2}"

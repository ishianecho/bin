# Sing-box

sing-box is managed by `sbctl` (a Rust binary, Linux only). It replaces the
local xray + sing-box pair with a single sing-box process: a tun inbound
captures all traffic, a mixed inbound serves HTTP/SOCKS, and the active node
is a vless + reality outbound. State lives in `~/.config/sbctl/state.json`;
`~/.config/sbctl/config.json` is generated output — do not edit it by hand.

## Commands

```sh
sbctl init
sbctl node add mynode 'vless://uuid@host:443?security=reality&sni=example.com&fp=chrome&pbk=PUBLICKEY&sid=SHORTID&type=tcp&flow=xtls-rprx-vision#mynode'
sbctl node pull https://example.com/subscription
sbctl node list
sbctl node use mynode
sbctl node set mynode --port 8443
sbctl node set mynode port 8443 sni example.com
sbctl node rm <tag>...
sbctl node rn mynode newname
sbctl node show [tag] [--link]

sbctl route add direct|proxy|block domain:example.com
sbctl route add proxy geosite:netflix
sbctl route add block 1.2.3.4
sbctl route list
sbctl route rm <matcher>...

sbctl port set --mixed 20808
sbctl mesh import ~/src/vpn/generated/sing-box-laptop.json
sbctl mesh on
sbctl mesh off
sbctl status
sbctl test
sbctl start
sbctl stop
sbctl restart
sbctl log [-n lines] [-f]
```

Only vless + reality/tls + xtls-rprx-vision is supported. Custom routes are
evaluated before all built-in rules (built-ins: QUIC reject, private →
direct, geosite-cn → direct, geoip-cn → direct; final → the active node).
CN domains resolve via 223.5.5.5 through the direct outbound, everything
else via 1.1.1.1 through the node. Mutating commands regenerate the config,
validate it with `sing-box check`, install it to `/etc/sing-box/config.json`
(`sudo install -m 0640 -o root -g sing-box`), and restart the service when
running (`--no-restart` skips). `/etc/sing-box/` must contain only that
file — the distro unit loads the whole directory.

`sbctl init` is a one-time migration: it imports nodes, routes, and the
active tag from `~/.config/xray/state.json` and sets the mixed port to the
old socks port, so the proxy environment keeps working unchanged. The tproxy
port and any certificate pins are dropped (sing-box has no pin equivalent).

`sbctl start` starts the service and points the proxy environment at the
mixed port (`proxy set`, see [`PROXY`](PROXY.md)); `sbctl stop` stops the
service, clears the environment (`proxy clear`), and appends a
`Host github.com` + `ProxyJump none` override to `~/.ssh/config.local`,
since the jump host is only reachable through the proxy; `sbctl start`
removes it.

## Mesh

`sbctl mesh import <path>` extracts the wireguard endpoint (address,
private key, peer public key, port, subnet) from a vpn-repo generated
config and stores it in the state; `mesh on` enables it, `mesh off`
disables it — no re-import needed. The endpoint is carried inside the
vless reality tunnel (peer 127.0.0.1, detour = the active node), so mesh
traffic needs no extra ports. Stacked configs (`-C /etc/sing-box`) are not
used; the generated file is self-contained.

## Switchover from xrayctl

Stop xray cleanly before enabling sing-box — leftover tproxy fwmark rules
conflict with the tun strict_route:

```sh
xrayctl stop && ip rule show
sudo systemctl disable --now xray
sbctl init && sbctl test
sudo systemctl enable sing-box && sbctl start
```

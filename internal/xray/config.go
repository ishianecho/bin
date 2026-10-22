package xray

import (
	"fmt"
	"runtime"
	"strings"
)

type generatedConfig struct {
	Log       logConfig        `json:"log"`
	DNS       dnsConfig        `json:"dns"`
	Inbounds  []map[string]any `json:"inbounds"`
	Outbounds []map[string]any `json:"outbounds"`
	Routing   routingConfig    `json:"routing"`
}

type logConfig struct {
	Loglevel string `json:"loglevel"`
	Access   string `json:"access,omitempty"`
}

type dnsConfig struct {
	Hosts   map[string][]string `json:"hosts"`
	Servers []any               `json:"servers"`
}

type routingConfig struct {
	DomainStrategy string           `json:"domainStrategy"`
	Rules          []map[string]any `json:"rules"`
}

func BuildConfig(state *State) (*generatedConfig, error) {
	if state.Active == "" {
		return nil, fmt.Errorf("No active node; run: xrayctl node use <tag>")
	}
	node := state.FindNode(state.Active)
	if node == nil {
		return nil, fmt.Errorf("Active node '%s' not found in %s", state.Active, StatePath())
	}

	config := &generatedConfig{
		Log: logConfig{Loglevel: "info", Access: "none"},
		DNS: dnsConfig{
			Hosts: map[string][]string{
				"dns.google":         {"8.8.8.8", "8.8.4.4"},
				"dns.alidns.com":     {"223.5.5.5", "223.6.6.6"},
				"one.one.one.one":    {"1.1.1.1", "1.0.0.1"},
				"cloudflare-dns.com": {"104.16.249.249", "104.16.248.249"},
				"doh.pub":            {"1.12.12.12", "120.53.53.53"},
				"dot.pub":            {"1.12.12.12", "120.53.53.53"},
			},
			Servers: []any{
				map[string]any{
					"address":      "https://dns.alidns.com/dns-query",
					"domains":      []string{"geosite:private", "geosite:cn"},
					"skipFallback": true,
				},
				map[string]any{
					"address":      "223.5.5.5",
					"domains":      []string{"full:dns.alidns.com", "full:cloudflare-dns.com"},
					"skipFallback": true,
				},
				"https://cloudflare-dns.com/dns-query",
			},
		},
		Inbounds:  inbounds(state),
		Outbounds: outbounds(node, state.Active),
		Routing:   routingConfig{DomainStrategy: "AsIs", Rules: routingRules(state, state.Active)},
	}
	return config, nil
}

func inbounds(state *State) []map[string]any {
	socks := map[string]any{
		"tag":      "socks",
		"port":     state.Ports.Socks,
		"listen":   "0.0.0.0",
		"protocol": "mixed",
		"sniffing": map[string]any{
			"enabled":      true,
			"destOverride": []string{"http", "tls"},
			"routeOnly":    false,
		},
		"settings": map[string]any{
			"auth":             "noauth",
			"udp":              true,
			"allowTransparent": false,
		},
	}
	if runtime.GOOS == "windows" {
		tun := map[string]any{
			"tag":      "tun",
			"protocol": "tun",
			"settings": map[string]any{
				"name":                   "XrayTun",
				"desc":                   "Xray",
				"gateway":                []string{"172.19.0.1/30"},
				"dns":                    []string{"223.5.5.5", "1.1.1.1"},
				"autoSystemRoutingTable": []string{"0.0.0.0/0"},
				"autoOutboundsInterface": "auto",
			},
		}
		return []map[string]any{socks, tun}
	}
	if runtime.GOOS == "linux" {
		tproxy := map[string]any{
			"tag":      "tproxy",
			"port":     state.Ports.Tproxy,
			"listen":   "0.0.0.0",
			"protocol": "tunnel",
			"settings": map[string]any{
				"network":        "tcp,udp",
				"followRedirect": true,
			},
			"streamSettings": map[string]any{
				"sockopt": map[string]any{"tproxy": "tproxy"},
			},
			"sniffing": map[string]any{
				"enabled":      true,
				"destOverride": []string{"http", "tls"},
			},
		}
		return []map[string]any{socks, tproxy}
	}
	return []map[string]any{socks}
}

func outbounds(node *Node, tag string) []map[string]any {
	security := node.Security
	if security == "" {
		security = "reality"
	}
	stream := map[string]any{
		"network":  "tcp",
		"security": security,
	}
	if security == "tls" {
		tlsSettings := map[string]any{
			"serverName":  node.SNI,
			"fingerprint": "chrome",
		}
		if node.Pin != "" {
			tlsSettings["pinnedPeerCertChainSha256"] = []string{node.Pin}
		}
		stream["tlsSettings"] = tlsSettings
	} else {
		stream["realitySettings"] = map[string]any{
			"serverName":  node.SNI,
			"fingerprint": "chrome",
			"show":        false,
			"publicKey":   node.PublicKey,
			"shortId":     node.ShortID,
			"spiderX":     "",
		}
	}
	vless := map[string]any{
		"tag":      tag,
		"protocol": "vless",
		"settings": map[string]any{
			"vnext": []map[string]any{
				{
					"address": node.Address,
					"port":    node.Port,
					"users": []map[string]any{
						{
							"id":         node.UUID,
							"email":      "t@t.tt",
							"security":   "auto",
							"encryption": "none",
							"flow":       "xtls-rprx-vision",
						},
					},
				},
			},
		},
		"streamSettings": stream,
		"mux": map[string]any{
			"enabled":     false,
			"concurrency": -1,
		},
	}
	if runtime.GOOS == "linux" {
		stream["sockopt"] = map[string]any{"mark": 255}
	}

	direct := map[string]any{
		"tag":      "direct",
		"protocol": "freedom",
	}
	if runtime.GOOS == "linux" {
		direct["streamSettings"] = map[string]any{"sockopt": map[string]any{"mark": 255}}
	}

	block := map[string]any{
		"tag":      "block",
		"protocol": "blackhole",
	}
	return []map[string]any{vless, direct, block}
}

func routingRules(state *State, tag string) []map[string]any {
	rules := make([]map[string]any, 0, len(state.Routes)+7)
	for _, route := range state.Routes {
		rule := map[string]any{
			"type":        "field",
			"network":     "tcp,udp",
			"outboundTag": route.Action,
		}
		if route.Action == "proxy" {
			rule["outboundTag"] = tag
		}
		if isDomainMatcher(route.Matcher) {
			rule["domain"] = []string{route.Matcher}
		} else {
			rule["ip"] = []string{route.Matcher}
		}
		rules = append(rules, rule)
	}
	rules = append(rules,
		map[string]any{"type": "field", "port": "443", "network": "udp", "outboundTag": "block"},
		map[string]any{"type": "field", "outboundTag": tag, "domain": []string{"geosite:google"}},
		map[string]any{"type": "field", "outboundTag": "direct", "ip": []string{"geoip:private"}},
		map[string]any{"type": "field", "outboundTag": "direct", "domain": []string{"geosite:private"}},
		map[string]any{"type": "field", "outboundTag": "direct", "domain": []string{"domain:alidns.com", "domain:doh.pub", "domain:dot.pub", "domain:360.cn", "domain:onedns.net"}},
		map[string]any{"type": "field", "outboundTag": "direct", "ip": []string{"geoip:cn"}},
		map[string]any{"type": "field", "outboundTag": "direct", "domain": []string{"geosite:cn"}},
	)
	return rules
}

func isDomainMatcher(matcher string) bool {
	for _, prefix := range []string{"domain:", "geosite:", "full:", "regexp:"} {
		if strings.HasPrefix(matcher, prefix) {
			return true
		}
	}
	return false
}

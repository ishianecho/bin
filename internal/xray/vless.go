package xray

import (
	"fmt"
	"strconv"
	"strings"
)

func ParseVlessLink(link string) (Node, error) {
	var node Node
	if !strings.HasPrefix(link, "vless://") {
		return node, fmt.Errorf("Expected a vless:// link")
	}
	rest := link[len("vless://"):]

	if index := strings.Index(rest, "#"); index >= 0 {
		node.Tag = percentDecode(rest[index+1:])
		rest = rest[:index]
	}
	query := ""
	if index := strings.Index(rest, "?"); index >= 0 {
		query = rest[index+1:]
		rest = rest[:index]
	}

	uuid := rest
	if index := strings.LastIndex(rest, "@"); index >= 0 {
		uuid = rest[:index]
		rest = rest[index+1:]
	}
	node.UUID = uuid
	address := rest
	port := ""
	if index := strings.LastIndex(rest, ":"); index >= 0 {
		address = rest[:index]
		port = rest[index+1:]
	}
	node.Address = strings.Trim(address, "[]")

	var security, network, flow, sni, publicKey, shortID, pin string
	for _, pair := range strings.Split(query, "&") {
		if pair == "" {
			continue
		}
		key := pair
		value := ""
		if index := strings.Index(pair, "="); index >= 0 {
			key = pair[:index]
			value = percentDecode(pair[index+1:])
		}
		switch key {
		case "security":
			security = value
		case "sni":
			sni = value
		case "pbk":
			publicKey = value
		case "sid":
			shortID = value
		case "type":
			network = value
		case "flow":
			flow = value
		case "pin":
			pin = value
		}
	}
	if security != "" && security != "reality" && security != "tls" {
		return node, fmt.Errorf("Only reality and tls security are supported")
	}
	if network != "" && network != "tcp" {
		return node, fmt.Errorf("Only the tcp transport is supported")
	}
	if flow != "" && flow != "xtls-rprx-vision" {
		return node, fmt.Errorf("Only the xtls-rprx-vision flow is supported")
	}

	node.SNI = sni
	node.PublicKey = publicKey
	node.ShortID = shortID
	node.Security = security
	node.Pin = pin
	node.Port = 0
	if port != "" {
		parsed, err := strconv.Atoi(port)
		if err != nil {
			return node, fmt.Errorf("node has an invalid port")
		}
		node.Port = parsed
	}
	return node, nil
}

func percentDecode(value string) string {
	var decoded strings.Builder
	for i := 0; i < len(value); i++ {
		if value[i] == '%' && i+3 <= len(value) {
			if n, err := strconv.ParseUint(value[i+1:i+3], 16, 8); err == nil {
				decoded.WriteByte(byte(n))
				i += 2
				continue
			}
		}
		decoded.WriteByte(value[i])
	}
	return decoded.String()
}

func ValidateNode(node *Node, name string) error {
	if node.UUID == "" {
		return fmt.Errorf("%s is missing a UUID", name)
	}
	if !isUUID(node.UUID) {
		return fmt.Errorf("%s has an invalid UUID", name)
	}
	if node.Address == "" {
		return fmt.Errorf("%s is missing an address", name)
	}
	if node.SNI == "" {
		return fmt.Errorf("%s is missing sni", name)
	}
	if node.Security != "" && node.Security != "reality" && node.Security != "tls" {
		return fmt.Errorf("%s has an unsupported security", name)
	}
	if node.Security != "tls" && node.PublicKey == "" {
		return fmt.Errorf("%s is missing a public key", name)
	}
	if node.Port == 0 {
		node.Port = 443
	}
	if node.Port < 1 || node.Port > 65535 {
		return fmt.Errorf("%s has an invalid port", name)
	}
	for _, value := range []string{node.Tag, node.Address, node.UUID, node.SNI, node.PublicKey, node.ShortID} {
		if strings.ContainsAny(value, " \t\n\r\"'\\") {
			return fmt.Errorf("%s must not contain whitespace, quotes, or backslashes", name)
		}
	}
	return nil
}

func isUUID(value string) bool {
	if len(value) != 36 {
		return false
	}
	for i, c := range value {
		if i == 8 || i == 13 || i == 18 || i == 23 {
			if c != '-' {
				return false
			}
			continue
		}
		if !(c >= '0' && c <= '9' || c >= 'a' && c <= 'f' || c >= 'A' && c <= 'F') {
			return false
		}
	}
	return true
}

func ValidateMatcher(matcher string) error {
	for _, prefix := range []string{"domain:", "geosite:", "full:", "regexp:", "geoip:"} {
		if strings.HasPrefix(matcher, prefix) {
			return nil
		}
	}
	for _, c := range matcher {
		if !(c >= '0' && c <= '9' || c >= 'a' && c <= 'f' || c >= 'A' && c <= 'F' || c == ':' || c == '.') {
			return fmt.Errorf("Invalid matcher: %s (use domain: geosite: full: regexp: geoip: or a bare IP/CIDR)", matcher)
		}
	}
	return nil
}

func ValidatePort(port int) error {
	if port < 1 || port > 65535 {
		return fmt.Errorf("Invalid port: %d", port)
	}
	return nil
}

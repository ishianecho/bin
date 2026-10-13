package proxy

import (
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"
)

type Config struct {
	HTTP    string
	SOCKS   string
	NoProxy string
}

func ConfigDir() string {
	if dir := os.Getenv("XDG_CONFIG_HOME"); dir != "" {
		return filepath.Join(dir, "proxy")
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return filepath.Join(".config", "proxy")
	}
	return filepath.Join(home, ".config", "proxy")
}

func ConfigPath() string {
	return filepath.Join(ConfigDir(), "config")
}

func Read() (Config, error) {
	var cfg Config
	data, err := os.ReadFile(ConfigPath())
	if err != nil {
		return cfg, fmt.Errorf("No proxy configuration found at %s", ConfigPath())
	}
	for _, line := range strings.Split(string(data), "\n") {
		for _, key := range []string{"PROXY_HTTP", "PROXY_SOCKS", "PROXY_NO_PROXY"} {
			prefix := key + "='"
			if strings.HasPrefix(line, prefix) && strings.HasSuffix(line, "'") {
				value := line[len(prefix) : len(line)-1]
				switch key {
				case "PROXY_HTTP":
					cfg.HTTP = value
				case "PROXY_SOCKS":
					cfg.SOCKS = value
				case "PROXY_NO_PROXY":
					cfg.NoProxy = value
				}
			}
		}
	}
	return cfg, nil
}

func Write(cfg Config) error {
	if strings.ContainsAny(cfg.NoProxy, "\r\n\x00") {
		return fmt.Errorf("--no-proxy must be a single comma-separated line")
	}
	dir := ConfigDir()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	temporary, err := os.CreateTemp(dir, "config.*")
	if err != nil {
		return err
	}
	name := temporary.Name()
	defer os.Remove(name)
	content := fmt.Sprintf("PROXY_HTTP='%s'\nPROXY_SOCKS='%s'\nPROXY_NO_PROXY='%s'\n", cfg.HTTP, cfg.SOCKS, cfg.NoProxy)
	if _, err := temporary.WriteString(content); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Chmod(0o600); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	return os.Rename(name, ConfigPath())
}

func Validate(value, kind string) error {
	if value == "" {
		return nil
	}
	if strings.ContainsAny(value, " \t\n\r\"'\\") {
		return fmt.Errorf("%s must not contain whitespace, quotes, or backslashes", kind)
	}
	parsed, err := url.Parse(value)
	scheme := ""
	if err == nil {
		scheme = parsed.Scheme
	}
	allowed := map[string][]string{
		"HTTP_PROXY":  {"http", "https"},
		"SOCKS_PROXY": {"socks5", "socks5h"},
	}
	permitted := false
	for _, s := range allowed[kind] {
		if scheme == s {
			permitted = true
		}
	}
	if !permitted {
		return fmt.Errorf("%s must use one of: %s://", kind, strings.Join(allowed[kind], "://, "))
	}
	return nil
}

func (c Config) DefaultNoProxy() string {
	if c.NoProxy != "" {
		return c.NoProxy
	}
	return "localhost,127.0.0.1,::1"
}

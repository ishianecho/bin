package proxy

import "os"

func Apply(cfg Config) error {
	entries := map[string]string{}
	if cfg.HTTP != "" {
		entries["HTTP_PROXY"] = cfg.HTTP
		entries["HTTPS_PROXY"] = cfg.HTTP
		entries["http_proxy"] = cfg.HTTP
		entries["https_proxy"] = cfg.HTTP
	} else {
		for _, key := range []string{"HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"} {
			os.Unsetenv(key)
		}
	}
	if cfg.SOCKS != "" {
		entries["ALL_PROXY"] = cfg.SOCKS
		entries["all_proxy"] = cfg.SOCKS
	} else {
		os.Unsetenv("ALL_PROXY")
		os.Unsetenv("all_proxy")
	}
	if cfg.HTTP != "" || cfg.SOCKS != "" {
		noProxy := cfg.DefaultNoProxy()
		entries["NO_PROXY"] = noProxy
		entries["no_proxy"] = noProxy
	} else {
		os.Unsetenv("NO_PROXY")
		os.Unsetenv("no_proxy")
	}
	for key, value := range entries {
		if err := os.Setenv(key, value); err != nil {
			return err
		}
	}
	return setUserEnvironment(cfg)
}

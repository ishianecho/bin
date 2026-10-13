//go:build !windows

package proxy

func setUserEnvironment(cfg Config) error {
	return nil
}

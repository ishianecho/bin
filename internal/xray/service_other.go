//go:build !linux && !windows

package xray

import "fmt"

func maybeRestart(noRestart bool) error {
	return nil
}

func ServiceState() string {
	return "unavailable"
}

func ServiceAction(action string) error {
	return fmt.Errorf("service management is not supported on this platform")
}

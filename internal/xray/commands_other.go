//go:build !linux && !windows

package xray

import "github.com/spf13/cobra"

func syncTproxyEnv(port int, changed bool) error {
	return nil
}

func platformCommands() []*cobra.Command {
	return nil
}

func maybeRestart(noRestart bool) error {
	return nil
}

func ServiceState() string {
	return "unavailable"
}

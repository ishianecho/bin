//go:build linux

package xray

import (
	"github.com/spf13/cobra"
)

func syncTproxyEnv(port int, changed bool) error {
	if !changed {
		return nil
	}
	return WriteTproxyEnv(port)
}

func platformCommands() []*cobra.Command {
	tproxy := &cobra.Command{
		Use:   "tproxy",
		Short: "Transparent-proxy commands",
	}
	tproxy.AddCommand(&cobra.Command{
		Use:   "status",
		Short: "Show transparent-proxy rule state",
		RunE: func(cmd *cobra.Command, args []string) error {
			TproxyStatus()
			return nil
		},
	})

	direct := &cobra.Command{
		Use:   "direct <cmd...>",
		Short: "Run a command outside the transparent proxy",
		Args:  cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			return Direct(args)
		},
	}
	return []*cobra.Command{tproxy, direct, newLogsCommand()}
}

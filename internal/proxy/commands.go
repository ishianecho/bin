package proxy

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

func NewCommand() *cobra.Command {
	root := &cobra.Command{
		Use:   "proxy",
		Short: "Manage and apply a proxy configuration",
		Long: "Manage a proxy configuration in " + ConfigPath() + " and apply it\n" +
			"through environment variables. Shell sessions and ssh pick the\n" +
			"file up directly; no per-tool configuration is written.",
		RunE: func(cmd *cobra.Command, args []string) error {
			return cmd.Help()
		},
		SilenceUsage: true,
	}
	root.AddCommand(newSetCommand(), newSSHCommand(), newClearCommand(), newShowCommand(), newSyncCommand())
	return root
}

func newSetCommand() *cobra.Command {
	var httpURL, socksURL, noProxy string
	command := &cobra.Command{
		Use:   "set [options] [HTTP_PROXY [SOCKS_PROXY]]",
		Short: "Save and apply a proxy configuration",
		Args:  cobra.MaximumNArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			for _, arg := range args {
				if httpURL == "" {
					httpURL = arg
				} else if socksURL == "" {
					socksURL = arg
				} else {
					return fmt.Errorf("unexpected argument: %s", arg)
				}
			}
			if httpURL == "" && socksURL == "" {
				return fmt.Errorf("set requires --http or --socks")
			}
			cfg := Config{HTTP: httpURL, SOCKS: socksURL, NoProxy: noProxy}
			if saved, err := Read(); err == nil {
				if cfg.HTTP == "" {
					cfg.HTTP = saved.HTTP
				}
				if cfg.SOCKS == "" {
					cfg.SOCKS = saved.SOCKS
				}
				if cfg.NoProxy == "" {
					cfg.NoProxy = saved.NoProxy
				}
			}
			if err := Validate(cfg.HTTP, "HTTP_PROXY"); err != nil {
				return err
			}
			if err := Validate(cfg.SOCKS, "SOCKS_PROXY"); err != nil {
				return err
			}
			if cfg.NoProxy == "" {
				cfg.NoProxy = "localhost,127.0.0.1,::1"
			}
			if err := Write(cfg); err != nil {
				return err
			}
			applied, err := Read()
			if err != nil {
				return err
			}
			if err := Apply(applied); err != nil {
				return err
			}
			fmt.Printf("Proxy configuration: %s\n", ConfigPath())
			return nil
		},
	}
	command.Flags().StringVar(&httpURL, "http", "", "HTTP/HTTPS proxy, for example http://127.0.0.1:10808")
	command.Flags().StringVar(&socksURL, "socks", "", "SOCKS proxy, for example socks5://127.0.0.1:10808")
	command.Flags().StringVar(&noProxy, "no-proxy", "", "comma-separated hosts which bypass the proxy")
	return command
}

func newSSHCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "ssh HOST PORT",
		Short: "Connect through the configured SOCKS/HTTP proxy",
		Args:  cobra.ExactArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			return Relay(args[0], args[1])
		},
	}
}

func newClearCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "clear",
		Short: "Remove all proxy settings",
		RunE: func(cmd *cobra.Command, args []string) error {
			if err := Write(Config{}); err != nil {
				return err
			}
			if err := Apply(Config{}); err != nil {
				return err
			}
			fmt.Printf("Proxy configuration: %s\n", ConfigPath())
			return nil
		},
	}
}

func newShowCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "show",
		Short: "Show the saved proxy configuration",
		RunE: func(cmd *cobra.Command, args []string) error {
			data, err := os.ReadFile(ConfigPath())
			if err != nil {
				return fmt.Errorf("No proxy configuration found at %s", ConfigPath())
			}
			cmd.OutOrStdout().Write(data)
			return nil
		},
	}
}

func newSyncCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "sync",
		Short: "Re-apply the saved configuration to the environment",
		RunE: func(cmd *cobra.Command, args []string) error {
			cfg, err := Read()
			if err != nil {
				return err
			}
			if err := Apply(cfg); err != nil {
				return err
			}
			fmt.Printf("Proxy configuration: %s\n", ConfigPath())
			return nil
		},
	}
}

package xray

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

var noRestart bool

func NewCommand() *cobra.Command {
	root := &cobra.Command{
		Use:   "xrayctl",
		Short: "State-driven manager for a local Xray service",
		Long: "State-driven manager for a local Xray service (vless + reality).\n" +
			"State lives in " + StatePath() + "; " + ConfigPath() + " is\n" +
			"generated output. Do not edit config.json by hand.",
		RunE: func(cmd *cobra.Command, args []string) error {
			return cmd.Help()
		},
		SilenceUsage: true,
	}
	root.PersistentFlags().BoolVar(&noRestart, "no-restart", false, "skip the service restart after a mutating command")
	root.AddCommand(newNodeCommand(), newRouteCommand(), newPortCommand())
	root.AddCommand(
		&cobra.Command{Use: "status", Short: "Show service state, active node, ports, and route count", RunE: runStatus},
		&cobra.Command{Use: "restart", Short: "Regenerate, validate, and restart the service", RunE: runRestart},
		&cobra.Command{Use: "test", Short: "Validate the generated configuration", RunE: runTest},
		&cobra.Command{Use: "init", Short: "First-run: import the env file, seed defaults", RunE: runInit},
	)
	root.AddCommand(platformCommands()...)
	return root
}

func newNodeCommand() *cobra.Command {
	command := &cobra.Command{
		Use:   "node",
		Short: "Manage nodes",
	}
	command.AddCommand(newNodeAddCommand(), newNodeRmCommand(), newNodeListCommand(), newNodeUseCommand(), newNodeShowCommand(), newNodeSetCommand())
	return command
}

func newNodeAddCommand() *cobra.Command {
	var flags nodeFlags
	var use bool
	command := &cobra.Command{
		Use:   "add [tag] (vless://link | --address H --port N --uuid U --sni S --public-key P --short-id I) [--use]",
		Short: "Add a vless+reality node; when the link carries a #tag the tag argument may be omitted",
		Args:  cobra.MaximumNArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			var tag, link string
			for _, arg := range args {
				if strings.HasPrefix(arg, "vless://") {
					if link != "" {
						return fmt.Errorf("Only one vless:// link is allowed")
					}
					link = arg
				} else if tag == "" {
					tag = arg
				} else {
					return fmt.Errorf("Unexpected argument: %s", arg)
				}
			}

			var node Node
			var err error
			if link != "" {
				node, err = ParseVlessLink(link)
				if err != nil {
					return err
				}
				if tag == "" {
					tag = node.Tag
				}
				if tag == "" {
					return fmt.Errorf("The link has no #tag; provide a tag explicitly")
				}
				node.Tag = tag
			} else {
				node = Node{
					Tag:       tag,
					Address:   flags.address,
					Port:      flags.port,
					UUID:      flags.uuid,
					SNI:       flags.sni,
					PublicKey: flags.publicKey,
					ShortID:   flags.shortID,
				}
			}
			if err := ValidateNode(&node, fmt.Sprintf("node '%s'", node.Tag)); err != nil {
				return err
			}

			state, err := Load()
			if err != nil {
				return err
			}
			if state.FindNode(node.Tag) != nil {
				return fmt.Errorf("Node '%s' already exists; use: xrayctl node set", node.Tag)
			}
			wasEmpty := state.Active == ""
			state.Nodes = append(state.Nodes, node)
			if wasEmpty || use {
				state.Active = node.Tag
			}
			if err := state.Save(); err != nil {
				return err
			}
			fmt.Printf("Added node %s\n", node.Tag)
			return Regenerate(state, noRestart)
		},
	}
	flags.add(command.Flags())
	command.Flags().BoolVar(&use, "use", false, "make the new node active")
	return command
}

func newNodeRmCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "rm <tag>",
		Short: "Remove a node",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			state, err := Load()
			if err != nil {
				return err
			}
			tag := args[0]
			if state.FindNode(tag) == nil {
				return fmt.Errorf("Node '%s' not found", tag)
			}
			nodes := state.Nodes[:0]
			for _, node := range state.Nodes {
				if node.Tag != tag {
					nodes = append(nodes, node)
				}
			}
			state.Nodes = nodes
			if state.Active == tag {
				state.Active = ""
			}
			if err := state.Save(); err != nil {
				return err
			}
			fmt.Printf("Removed node %s\n", tag)
			if state.Active != "" {
				return Regenerate(state, noRestart)
			}
			fmt.Fprintln(os.Stderr, "No active node; run: xrayctl node use <tag>")
			return nil
		},
	}
}

func newNodeListCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "list",
		Short: "List nodes (* marks the active node)",
		RunE: func(cmd *cobra.Command, args []string) error {
			state, err := Load()
			if err != nil {
				return err
			}
			for _, node := range state.Nodes {
				marker := ""
				if state.Active == node.Tag {
					marker = " *"
				}
				fmt.Printf("%s%s\t%s:%d\n", node.Tag, marker, node.Address, node.Port)
			}
			return nil
		},
	}
}

func newNodeUseCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "use <tag>",
		Short: "Set the active node",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			state, err := Load()
			if err != nil {
				return err
			}
			tag := args[0]
			if state.FindNode(tag) == nil {
				return fmt.Errorf("Node '%s' not found", tag)
			}
			state.Active = tag
			if err := state.Save(); err != nil {
				return err
			}
			fmt.Printf("Active node: %s\n", tag)
			return Regenerate(state, noRestart)
		},
	}
}

func newNodeShowCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "show [tag]",
		Short: "Show node details",
		Args:  cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			state, err := Load()
			if err != nil {
				return err
			}
			tag := state.Active
			if len(args) == 1 {
				tag = args[0]
			}
			if tag == "" {
				return fmt.Errorf("No active node; run: xrayctl node use <tag>")
			}
			node := state.FindNode(tag)
			if node == nil {
				return fmt.Errorf("Node '%s' not found", tag)
			}
			printJSON(node)
			return nil
		},
	}
}

type nodeFlags struct {
	address   string
	port      int
	uuid      string
	sni       string
	publicKey string
	shortID   string
}

func (f *nodeFlags) add(flags *pflag.FlagSet) {
	flags.StringVar(&f.address, "address", "", "server address")
	flags.IntVar(&f.port, "port", 0, "server port")
	flags.StringVar(&f.uuid, "uuid", "", "user UUID")
	flags.StringVar(&f.sni, "sni", "", "reality server name")
	flags.StringVar(&f.publicKey, "public-key", "", "reality public key")
	flags.StringVar(&f.shortID, "short-id", "", "reality short ID")
}

func newNodeSetCommand() *cobra.Command {
	var flags nodeFlags
	command := &cobra.Command{
		Use:   "set <tag> [--address H] [--port N] [--uuid U] [--sni S] [--public-key P] [--short-id I]",
		Short: "Update a node",
		Args:  cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			tag := args[0]
			for _, arg := range args[1:] {
				if !strings.HasPrefix(arg, "-") {
					return fmt.Errorf("Unexpected argument: %s", arg)
				}
			}
			state, err := Load()
			if err != nil {
				return err
			}
			node := state.FindNode(tag)
			if node == nil {
				return fmt.Errorf("Node '%s' not found", tag)
			}
			changed := cmd.Flags()
			set := func(name string, target *string) error {
				if !changed.Changed(name) {
					return nil
				}
				value, err := changed.GetString(name)
				if err != nil {
					return err
				}
				if strings.ContainsAny(value, " \t\n\r\"'\\") {
					return fmt.Errorf("Values must not contain whitespace, quotes, or backslashes")
				}
				*target = value
				return nil
			}
			if err := set("address", &node.Address); err != nil {
				return err
			}
			if err := set("uuid", &node.UUID); err != nil {
				return err
			}
			if err := set("sni", &node.SNI); err != nil {
				return err
			}
			if err := set("public-key", &node.PublicKey); err != nil {
				return err
			}
			if err := set("short-id", &node.ShortID); err != nil {
				return err
			}
			if changed.Changed("port") {
				if err := ValidatePort(flags.port); err != nil {
					return err
				}
				node.Port = flags.port
			}
			if err := state.Save(); err != nil {
				return err
			}
			fmt.Printf("Updated node %s\n", tag)
			if state.Active == tag {
				return Regenerate(state, noRestart)
			}
			return nil
		},
	}
	flags.add(command.Flags())
	return command
}

func newRouteCommand() *cobra.Command {
	command := &cobra.Command{
		Use:   "route",
		Short: "Manage routing rules (evaluated before all built-in rules)",
	}
	command.AddCommand(newRouteAddCommand(), newRouteRmCommand(), newRouteListCommand())
	return command
}

func newRouteAddCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "add direct|proxy|block <matcher...>",
		Short: "Add routing rules; matcher: domain:xxx.com, geosite:cn, full:example.com, regexp:pat, geoip:cn, or a bare IP/CIDR",
		Args:  cobra.MinimumNArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			action := args[0]
			if action != "direct" && action != "proxy" && action != "block" {
				return fmt.Errorf("Action must be direct, proxy, or block")
			}
			for _, matcher := range args[1:] {
				if err := ValidateMatcher(matcher); err != nil {
					return err
				}
			}
			state, err := Load()
			if err != nil {
				return err
			}
			for _, matcher := range args[1:] {
				for _, route := range state.Routes {
					if route.Matcher == matcher {
						return fmt.Errorf("Route '%s' already exists; run: xrayctl route rm %s first", matcher, matcher)
					}
				}
			}
			for _, matcher := range args[1:] {
				state.Routes = append(state.Routes, Route{Action: action, Matcher: matcher})
			}
			if err := state.Save(); err != nil {
				return err
			}
			fmt.Printf("Added %d route(s)\n", len(args)-1)
			return Regenerate(state, noRestart)
		},
	}
}

func newRouteRmCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "rm <matcher|index>",
		Short: "Remove a routing rule",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			state, err := Load()
			if err != nil {
				return err
			}
			target := args[0]
			if index, err := strconv.Atoi(target); err == nil {
				if index < 1 || index > len(state.Routes) {
					return fmt.Errorf("Index out of range: %s (1-%d)", target, len(state.Routes))
				}
				state.Routes = append(state.Routes[:index-1], state.Routes[index:]...)
			} else {
				found := false
				routes := state.Routes[:0]
				for _, route := range state.Routes {
					if route.Matcher == target {
						found = true
						continue
					}
					routes = append(routes, route)
				}
				state.Routes = routes
				if !found {
					return fmt.Errorf("Route not found: %s", target)
				}
			}
			if err := state.Save(); err != nil {
				return err
			}
			fmt.Printf("Removed route %s\n", target)
			return Regenerate(state, noRestart)
		},
	}
}

func newRouteListCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "list",
		Short: "List routing rules",
		RunE: func(cmd *cobra.Command, args []string) error {
			state, err := Load()
			if err != nil {
				return err
			}
			for i, route := range state.Routes {
				fmt.Printf("%d %s %s\n", i+1, route.Action, route.Matcher)
			}
			return nil
		},
	}
}

func newPortCommand() *cobra.Command {
	command := &cobra.Command{
		Use:   "port",
		Short: "Manage listening ports",
	}
	set := &cobra.Command{
		Use:   "set [--socks N] [--tproxy N]",
		Short: "Set the socks or tproxy port",
		RunE: func(cmd *cobra.Command, args []string) error {
			flags := cmd.Flags()
			if !flags.Changed("socks") && !flags.Changed("tproxy") {
				return fmt.Errorf("port set requires --socks or --tproxy")
			}
			state, err := Load()
			if err != nil {
				return err
			}
			currentSocks := state.Ports.Socks
			currentTproxy := state.Ports.Tproxy
			socks := currentSocks
			tproxy := currentTproxy
			if flags.Changed("socks") {
				socks, _ = flags.GetInt("socks")
			}
			if flags.Changed("tproxy") {
				tproxy, _ = flags.GetInt("tproxy")
				if runtime.GOOS == "windows" {
					fmt.Println("The tproxy port has no effect on Windows")
				}
			}
			if err := ValidatePort(socks); err != nil {
				return err
			}
			if err := ValidatePort(tproxy); err != nil {
				return err
			}
			if socks == tproxy {
				return fmt.Errorf("The socks and tproxy ports must differ")
			}
			state.Ports = Ports{Socks: socks, Tproxy: tproxy}
			if err := state.Save(); err != nil {
				return err
			}
			if err := syncTproxyEnv(tproxy, tproxy != currentTproxy); err != nil {
				return err
			}
			fmt.Printf("Ports: socks=%d tproxy=%d\n", socks, tproxy)
			if err := Regenerate(state, noRestart); err != nil {
				return err
			}
			if socks != currentSocks {
				syncProxyEnv(socks)
			}
			return nil
		},
	}
	set.Flags().Int("socks", 0, "socks port")
	set.Flags().Int("tproxy", 0, "tproxy port")
	command.AddCommand(set)
	return command
}

func runStatus(cmd *cobra.Command, args []string) error {
	state, err := Load()
	if err != nil {
		return err
	}
	fmt.Printf("service: %s\n", ServiceState())
	fmt.Printf("node: %s\n", state.Active)
	fmt.Printf("ports: socks=%d tproxy=%d\n", state.Ports.Socks, state.Ports.Tproxy)
	fmt.Printf("routes: %d custom\n", len(state.Routes))
	return nil
}

func runRestart(cmd *cobra.Command, args []string) error {
	state, err := Load()
	if err != nil {
		return err
	}
	return Regenerate(state, noRestart)
}

func runTest(cmd *cobra.Command, args []string) error {
	if _, err := os.Stat(ConfigPath()); err != nil {
		return fmt.Errorf("No generated config at %s; run: xrayctl init", ConfigPath())
	}
	if _, err := exec.LookPath("xray"); err != nil {
		return fmt.Errorf("xray not found")
	}
	command := exec.Command("xray", "run", "-test", "-c", ConfigPath())
	command.Stdout = os.Stdout
	command.Stderr = os.Stderr
	return command.Run()
}

func runInit(cmd *cobra.Command, args []string) error {
	if _, err := os.Stat(StatePath()); os.IsNotExist(err) {
		if _, err := os.Stat(EnvPath()); err == nil {
			return importEnv()
		}
		if _, err := Load(); err != nil {
			return err
		}
		fmt.Printf("Created empty state at %s\n", StatePath())
		fmt.Println("Add a node: xrayctl node add <tag> vless://...")
		return nil
	}
	state, err := Load()
	if err != nil {
		return err
	}
	return Regenerate(state, noRestart)
}

func importEnv() error {
	data, err := os.ReadFile(EnvPath())
	if err != nil {
		return err
	}
	values := map[string]string{}
	for _, line := range strings.Split(string(data), "\n") {
		if index := strings.Index(line, "="); index > 0 && strings.HasPrefix(line, "XRAY_") {
			values[line[:index]] = line[index+1:]
		}
	}
	if values["XRAY_ADDRESS"] == "" || values["XRAY_UUID"] == "" {
		return fmt.Errorf("%s lacks XRAY_ADDRESS or XRAY_UUID", EnvPath())
	}
	port := 443
	if values["XRAY_PORT"] != "" {
		port, err = strconv.Atoi(values["XRAY_PORT"])
		if err != nil {
			return fmt.Errorf("env node has an invalid port")
		}
	}
	node := Node{
		Tag:       values["XRAY_TAG"],
		Address:   values["XRAY_ADDRESS"],
		Port:      port,
		UUID:      values["XRAY_UUID"],
		SNI:       values["XRAY_SNI"],
		PublicKey: values["XRAY_PUBLIC_KEY"],
		ShortID:   values["XRAY_SHORT_ID"],
	}
	if err := ValidateNode(&node, "env node"); err != nil {
		return err
	}
	state, err := Load()
	if err != nil {
		return err
	}
	state.Nodes = []Node{node}
	state.Active = node.Tag
	state.Routes = []Route{
		{Action: "direct", Matcher: "domain:sourceforge.net"},
		{Action: "direct", Matcher: "domain:stackoverflow.com"},
	}
	if err := state.Save(); err != nil {
		return err
	}
	fmt.Printf("Imported node %s from %s\n", node.Tag, EnvPath())
	fmt.Printf("The env file is deprecated; state.json is authoritative and %s may be deleted\n", EnvPath())
	return nil
}

func printJSON(value any) {
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return
	}
	fmt.Println(string(data))
}

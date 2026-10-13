//go:build linux

package xray

import (
	"fmt"
	"os"
	"os/exec"
	"os/user"
	"strings"
	"syscall"
)

const (
	tproxyEnvFile = "/usr/local/etc/xray/tproxy.env"
	tproxyGroup   = "xray_tproxy"
)

func maybeRestart(noRestart bool) error {
	if noRestart {
		return nil
	}
	if _, err := exec.LookPath("systemctl"); err != nil {
		return nil
	}
	active := exec.Command("systemctl", "is-active", "--quiet", "xray")
	if err := active.Run(); err != nil {
		return nil
	}
	if _, err := exec.LookPath("sudo"); err != nil {
		return fmt.Errorf("sudo is required to restart xray")
	}
	restart := exec.Command("sudo", "systemctl", "restart", "xray")
	restart.Stdin = os.Stdin
	restart.Stdout = os.Stdout
	restart.Stderr = os.Stderr
	if err := restart.Run(); err != nil {
		return fmt.Errorf("failed to restart xray: %w", err)
	}
	fmt.Println("xray restarted")
	return nil
}

func ServiceState() string {
	if _, err := exec.LookPath("systemctl"); err != nil {
		return "unavailable"
	}
	output, err := exec.Command("systemctl", "is-active", "xray").Output()
	if err != nil {
		if len(output) == 0 {
			return "unknown"
		}
	}
	state := strings.TrimSpace(string(output))
	if state == "" {
		return "unknown"
	}
	return state
}

func WriteTproxyEnv(port int) error {
	if _, err := exec.LookPath("sudo"); err != nil {
		return fmt.Errorf("sudo is required to update %s", tproxyEnvFile)
	}
	command := exec.Command("sudo", "tee", tproxyEnvFile)
	command.Stdin = strings.NewReader(fmt.Sprintf("TPROXY_PORT=%d\n", port))
	command.Stdout = nil
	command.Stderr = os.Stderr
	if err := command.Run(); err != nil {
		return fmt.Errorf("failed to update %s", tproxyEnvFile)
	}
	return nil
}

func TproxyStatus() {
	fmt.Printf("service: %s\n", ServiceState())
	rules, err := exec.Command("ip", "rule", "show").Output()
	if err == nil && strings.Contains(string(rules), "fwmark 0x1 lookup 100") {
		fmt.Println("policy routing: active")
	} else {
		fmt.Println("policy routing: inactive")
	}
	if err := probeChain("mangle", "XRAY"); err == nil {
		fmt.Println("mangle XRAY chain: present")
	} else {
		fmt.Println("mangle XRAY chain: absent (or sudo credentials required)")
	}
	if err := probeChain("nat", "XRAY_DNS"); err == nil {
		fmt.Println("nat XRAY_DNS chain: present")
	} else {
		fmt.Println("nat XRAY_DNS chain: absent (or sudo credentials required)")
	}
}

func probeChain(table, chain string) error {
	return exec.Command("sudo", "-n", "iptables", "-t", table, "-S", chain).Run()
}

func Direct(args []string) error {
	if len(args) == 0 {
		return fmt.Errorf("direct requires a command")
	}
	if inGroup(tproxyGroup) {
		argv := []string{"sg", tproxyGroup, "-c", strings.Join(args, " ")}
		return syscall.Exec(mustLookPath("sg"), argv, os.Environ())
	}
	if _, err := exec.LookPath("sudo"); err != nil {
		return fmt.Errorf("Join %s and re-login, or install sudo", tproxyGroup)
	}
	argv := append([]string{"sudo", "-g", tproxyGroup, "--"}, args...)
	return syscall.Exec(mustLookPath("sudo"), argv, os.Environ())
}

func inGroup(name string) bool {
	current, err := user.Current()
	if err != nil {
		return false
	}
	ids, err := current.GroupIds()
	if err != nil {
		return false
	}
	for _, id := range ids {
		group, err := user.LookupGroupId(id)
		if err == nil && group.Name == name {
			return true
		}
	}
	return false
}

func mustLookPath(name string) string {
	path, err := exec.LookPath(name)
	if err != nil {
		fmt.Fprintf(os.Stderr, "xrayctl: %s not found\n", name)
		os.Exit(1)
	}
	return path
}

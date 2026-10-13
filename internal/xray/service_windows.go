//go:build windows

package xray

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"github.com/spf13/cobra"
	"golang.org/x/sys/windows"
	"golang.org/x/sys/windows/svc"
	"golang.org/x/sys/windows/svc/mgr"
)

const serviceName = "xray"

func maybeRestart(noRestart bool) error {
	if noRestart {
		return nil
	}
	if serviceState() != "Running" {
		return nil
	}
	nssm, err := nssmPath()
	if err != nil {
		return err
	}
	if err := runNssm(nssm, "restart", serviceName); err != nil {
		return err
	}
	fmt.Println("xray restarted")
	return nil
}

func ServiceState() string {
	return serviceState()
}

func serviceState() string {
	manager, err := mgr.Connect()
	if err != nil {
		return "unknown"
	}
	defer manager.Disconnect()
	service, err := manager.OpenService(serviceName)
	if err != nil {
		return "not installed"
	}
	defer service.Close()
	status, err := service.Query()
	if err != nil {
		return "unknown"
	}
	switch status.State {
	case svc.Stopped:
		return "Stopped"
	case svc.StartPending:
		return "StartPending"
	case svc.StopPending:
		return "StopPending"
	case svc.Running:
		return "Running"
	case svc.ContinuePending:
		return "ContinuePending"
	case svc.PausePending:
		return "PausePending"
	case svc.Paused:
		return "Paused"
	default:
		return "unknown"
	}
}

func syncTproxyEnv(port int, changed bool) error {
	return nil
}

func platformCommands() []*cobra.Command {
	return []*cobra.Command{newServiceCommand()}
}

func newServiceCommand() *cobra.Command {
	command := &cobra.Command{
		Use:   "service",
		Short: "Manage the xray service (NSSM; requires an elevated terminal)",
	}
	command.AddCommand(
		serviceAction("install", installService, true),
		serviceAction("uninstall", uninstallService, true),
		serviceAction("start", nil, true),
		serviceAction("stop", nil, true),
		serviceAction("restart", nil, true),
		serviceAction("status", nil, false),
	)
	return command
}

func serviceAction(name string, run func([]string) error, elevated bool) *cobra.Command {
	return &cobra.Command{
		Use:   name,
		Short: name + " the xray service",
		RunE: func(cmd *cobra.Command, args []string) error {
			if elevated {
				if err := requireAdmin(); err != nil {
					return err
				}
			}
			if name == "status" {
				fmt.Println(ServiceState())
				return nil
			}
			if run != nil {
				return run(args)
			}
			nssm, err := nssmPath()
			if err != nil {
				return err
			}
			return runNssm(nssm, name)
		},
	}
}

func installService(args []string) error {
	nssm, err := nssmPath()
	if err != nil {
		return err
	}
	exe, err := exec.LookPath("xray")
	if err != nil {
		return fmt.Errorf("xray not found")
	}
	if err := runNssm(nssm, "install", serviceName, exe, "run -c "+ConfigPath()); err != nil {
		return err
	}
	if err := runNssm(nssm, "set", serviceName, "AppDirectory", filepath.Dir(exe)); err != nil {
		return err
	}
	return runNssm(nssm, "start")
}

func uninstallService(args []string) error {
	nssm, err := nssmPath()
	if err != nil {
		return err
	}
	if err := runNssm(nssm, "stop"); err != nil {
		return err
	}
	return runNssm(nssm, "remove", serviceName, "confirm")
}

func runNssm(nssm string, args ...string) error {
	command := exec.Command(nssm, args...)
	command.Stdout = os.Stdout
	command.Stderr = os.Stderr
	if err := command.Run(); err != nil {
		return fmt.Errorf("nssm %s failed", args[0])
	}
	return nil
}

func nssmPath() (string, error) {
	if path, err := exec.LookPath("nssm"); err == nil {
		return path, nil
	}
	root := os.Getenv("SCOOP")
	if root == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return "", err
		}
		root = filepath.Join(home, "scoop")
	}
	candidate := filepath.Join(root, "apps", "nssm", "current", "nssm.exe")
	if _, err := os.Stat(candidate); err == nil {
		return candidate, nil
	}
	return "", fmt.Errorf("nssm not found; install it with the nssm installer or: scoop install nssm")
}

func requireAdmin() error {
	token := windows.GetCurrentProcessToken()
	defer token.Close()
	if !token.IsElevated() {
		return fmt.Errorf("Service management requires an elevated (administrator) terminal")
	}
	return nil
}

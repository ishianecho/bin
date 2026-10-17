package xray

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
)

func Regenerate(state *State, noRestart bool) error {
	config, err := BuildConfig(state)
	if err != nil {
		return err
	}
	data, err := json.MarshalIndent(config, "", "  ")
	if err != nil {
		return err
	}
	newPath := filepath.Join(ConfigDir(), "config.new.json")
	if err := os.WriteFile(newPath, append(data, '\n'), 0o644); err != nil {
		return err
	}
	if _, err := exec.LookPath("xray"); err == nil {
		command := exec.Command("xray", "run", "-test", "-c", newPath)
		stderr, err := command.StderrPipe()
		if err != nil {
			return err
		}
		if err := command.Start(); err != nil {
			return err
		}
		message, _ := io.ReadAll(stderr)
		if err := command.Wait(); err != nil {
			os.Remove(newPath)
			return fmt.Errorf("Generated configuration failed 'xray run -test'; the previous config is kept: %s", strings.TrimSpace(string(message)))
		}
	}
	if err := os.Rename(newPath, ConfigPath()); err != nil {
		return err
	}
	fmt.Printf("Generated %s\n", ConfigPath())
	return maybeRestart(noRestart)
}

func syncProxyEnv(socksPort int) {
	name := "proxy"
	if runtime.GOOS == "windows" {
		name = "proxy.exe"
	}
	path, err := exec.LookPath(name)
	if err != nil {
		home, err := os.UserHomeDir()
		if err != nil {
			return
		}
		path = filepath.Join(home, ".local", "bin", name)
		if _, err := os.Stat(path); err != nil {
			return
		}
	}
	command := exec.Command(path, "set",
		"--http", fmt.Sprintf("http://127.0.0.1:%d", socksPort),
		"--socks", fmt.Sprintf("socks5://127.0.0.1:%d", socksPort),
	)
	command.Stdout = os.Stdout
	command.Stderr = os.Stderr
	command.Run()
}

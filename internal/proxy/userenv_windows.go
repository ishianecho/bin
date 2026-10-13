//go:build windows

package proxy

import (
	"fmt"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows/registry"
)

const (
	hwndBroadcast    = 0xffff
	wmSettingChange  = 0x001A
	smwtoAbortIfHung = 0x0002
)

var (
	user32             = syscall.NewLazyDLL("user32.dll")
	sendMessageTimeout = user32.NewProc("SendMessageTimeoutW")
)

func setUserEnvironment(cfg Config) error {
	key, err := registry.OpenKey(registry.CURRENT_USER, `Environment`, registry.SET_VALUE)
	if err != nil {
		return fmt.Errorf("set user environment: %w", err)
	}
	defer key.Close()

	entries := map[string]string{
		"HTTP_PROXY":  cfg.HTTP,
		"HTTPS_PROXY": cfg.HTTP,
		"ALL_PROXY":   cfg.SOCKS,
	}
	if cfg.HTTP != "" || cfg.SOCKS != "" {
		entries["NO_PROXY"] = cfg.DefaultNoProxy()
	} else {
		entries["NO_PROXY"] = ""
	}
	for name, value := range entries {
		var err error
		if value == "" {
			err = key.DeleteValue(name)
			if err == registry.ErrNotExist {
				err = nil
			}
		} else {
			err = key.SetStringValue(name, value)
		}
		if err != nil {
			return fmt.Errorf("set user environment %s: %w", name, err)
		}
	}

	environment, err := syscall.UTF16PtrFromString("Environment")
	if err != nil {
		return err
	}
	sendMessageTimeout.Call(
		uintptr(hwndBroadcast),
		uintptr(wmSettingChange),
		0,
		uintptr(unsafe.Pointer(environment)),
		uintptr(smwtoAbortIfHung),
		5000,
		0,
	)
	return nil
}

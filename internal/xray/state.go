package xray

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

type Node struct {
	Tag           string `json:"tag"`
	Address       string `json:"address"`
	Port          int    `json:"port"`
	UUID          string `json:"uuid"`
	SNI           string `json:"sni"`
	PublicKey     string `json:"publicKey"`
	ShortID       string `json:"shortId"`
	Security      string `json:"security,omitempty"`
	AllowInsecure bool   `json:"allowInsecure,omitempty"`
}

type Route struct {
	Action  string `json:"action"`
	Matcher string `json:"matcher"`
}

type Ports struct {
	Socks  int `json:"socks"`
	Tproxy int `json:"tproxy"`
}

type State struct {
	Version int     `json:"version"`
	Active  string  `json:"active"`
	Ports   Ports   `json:"ports"`
	Nodes   []Node  `json:"nodes"`
	Routes  []Route `json:"routes"`
}

func ConfigDir() string {
	if dir := os.Getenv("XDG_CONFIG_HOME"); dir != "" {
		return filepath.Join(dir, "xray")
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return filepath.Join(".config", "xray")
	}
	return filepath.Join(home, ".config", "xray")
}

func StatePath() string {
	return filepath.Join(ConfigDir(), "state.json")
}

func ConfigPath() string {
	return filepath.Join(ConfigDir(), "config.json")
}

func EnvPath() string {
	return filepath.Join(ConfigDir(), "env")
}

func Load() (*State, error) {
	path := StatePath()
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		state := &State{
			Version: 1,
			Ports:   Ports{Socks: 10808, Tproxy: 10800},
		}
		if err := state.Save(); err != nil {
			return nil, err
		}
		return state, nil
	}
	if err != nil {
		return nil, err
	}
	var state State
	if err := json.Unmarshal(data, &state); err != nil {
		return nil, fmt.Errorf("Invalid state file at %s", path)
	}
	if state.Nodes == nil {
		state.Nodes = []Node{}
	}
	if state.Routes == nil {
		state.Routes = []Route{}
	}
	return &state, nil
}

func (s *State) Save() error {
	if err := os.MkdirAll(ConfigDir(), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return err
	}
	temporary, err := os.CreateTemp(ConfigDir(), "state.*.json")
	if err != nil {
		return err
	}
	name := temporary.Name()
	defer os.Remove(name)
	if _, err := temporary.Write(append(data, '\n')); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Chmod(0o600); err != nil {
		temporary.Close()
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	return os.Rename(name, StatePath())
}

func (s *State) FindNode(tag string) *Node {
	for i := range s.Nodes {
		if s.Nodes[i].Tag == tag {
			return &s.Nodes[i]
		}
	}
	return nil
}

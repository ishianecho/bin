package main

import (
	"os"

	"github.com/ishianecho/bin/internal/proxy"
)

func main() {
	if err := proxy.NewCommand().Execute(); err != nil {
		os.Exit(1)
	}
}

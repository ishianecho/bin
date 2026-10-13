package main

import (
	"os"

	"github.com/ishianecho/bin/internal/xray"
)

func main() {
	if err := xray.NewCommand().Execute(); err != nil {
		os.Exit(1)
	}
}

package proxy

import (
	"bufio"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"strings"

	"golang.org/x/net/proxy"
)

func Relay(host, port string) error {
	target := net.JoinHostPort(host, port)

	proxyURL := ""
	if cfg, err := Read(); err == nil {
		proxyURL = cfg.SOCKS
	}

	conn, err := dial(target, proxyURL)
	if err != nil {
		return err
	}
	defer conn.Close()

	stdoutDone := make(chan error, 1)
	stdinDone := make(chan error, 1)
	go func() {
		_, err := io.Copy(os.Stdout, conn)
		stdoutDone <- err
	}()
	go func() {
		_, err := io.Copy(conn, os.Stdin)
		stdinDone <- err
	}()

	<-stdinDone
	return <-stdoutDone
}

func dial(target, proxyURL string) (net.Conn, error) {
	if proxyURL == "" {
		return net.Dial("tcp", target)
	}
	parsed, err := url.Parse(proxyURL)
	if err != nil {
		return nil, fmt.Errorf("unsupported proxy URL: %s", proxyURL)
	}
	address := parsed.Host
	if !strings.Contains(address, ":") {
		return nil, fmt.Errorf("unsupported proxy URL: %s", proxyURL)
	}
	switch parsed.Scheme {
	case "socks5", "socks5h":
		dialer, err := proxy.SOCKS5("tcp", address, nil, proxy.Direct)
		if err != nil {
			return nil, err
		}
		return dialer.Dial("tcp", target)
	case "http", "https":
		conn, err := net.Dial("tcp", address)
		if err != nil {
			return nil, err
		}
		request := &http.Request{
			Method: http.MethodConnect,
			URL:    &url.URL{Opaque: target},
			Host:   target,
			Header: make(http.Header),
		}
		if err := request.Write(conn); err != nil {
			conn.Close()
			return nil, err
		}
		response, err := http.ReadResponse(bufio.NewReader(conn), request)
		if err != nil {
			conn.Close()
			return nil, err
		}
		if response.StatusCode != http.StatusOK {
			conn.Close()
			return nil, fmt.Errorf("proxy CONNECT failed: %s", response.Status)
		}
		return conn, nil
	default:
		return nil, fmt.Errorf("unsupported proxy URL: %s", proxyURL)
	}
}

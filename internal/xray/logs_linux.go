//go:build linux

package xray

import (
	"bufio"
	"bytes"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"regexp"
	"strconv"
	"strings"
	"syscall"

	"github.com/spf13/cobra"
)

var logDetourRe = regexp.MustCompile(`\[(\d+)\] app/dispatcher: taking detour \[(.+?)\] for \[(.+?)\]`)
var logDefaultRouteRe = regexp.MustCompile(`\[(\d+)\] app/dispatcher: default route for (\S+)`)
var logInboundRe = regexp.MustCompile(`\[(\d+)\] proxy/(socks|dokodemo):`)
var logDokodemoSourceRe = regexp.MustCompile(`\[(\d+)\] proxy/dokodemo: received request for (\S+)`)
var logSocksSourceRe = regexp.MustCompile(`\[(\d+)\] proxy/socks: client UDP connection from (\S+)`)
var logTimestampRe = regexp.MustCompile(`^(\d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2}(?:\.\d+)?) `)
var logInboundTags = map[string]string{"socks": "socks", "dokodemo": "tproxy"}

type logSession struct {
	inbound string
	from    string
}

func logHost(addr string) string {
	if host, _, err := net.SplitHostPort(addr); err == nil {
		return host
	}
	return addr
}

func filterLogLines(input io.Reader, output io.Writer, defaultTag string) {
	scanner := bufio.NewScanner(input)
	scanner.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	sessions := make(map[string]logSession)
	for scanner.Scan() {
		line := scanner.Text()
		prefix := ""
		if m := logTimestampRe.FindStringSubmatch(line); m != nil {
			prefix = m[1] + " "
		}
		if m := logDokodemoSourceRe.FindStringSubmatch(line); m != nil {
			if len(sessions) > 8192 {
				sessions = make(map[string]logSession)
			}
			sessions[m[1]] = logSession{inbound: "tproxy", from: logHost(m[2])}
			continue
		}
		if m := logSocksSourceRe.FindStringSubmatch(line); m != nil {
			if len(sessions) > 8192 {
				sessions = make(map[string]logSession)
			}
			sessions[m[1]] = logSession{inbound: "socks", from: logHost(m[2])}
			continue
		}
		if m := logInboundRe.FindStringSubmatch(line); m != nil {
			if len(sessions) > 8192 {
				sessions = make(map[string]logSession)
			}
			sessions[m[1]] = logSession{inbound: logInboundTags[m[2]]}
			continue
		}
		if m := logDetourRe.FindStringSubmatch(line); m != nil {
			session := sessions[m[1]]
			if session.inbound == "" {
				session.inbound = "tproxy"
			}
			if session.from == "" {
				session.from = "-"
			}
			fmt.Fprintf(output, "%sfrom %s to %s [%s -> %s]\n", prefix, session.from, m[3], session.inbound, m[2])
			continue
		}
		if m := logDefaultRouteRe.FindStringSubmatch(line); m != nil {
			session := sessions[m[1]]
			if session.inbound == "" {
				session.inbound = "tproxy"
			}
			if session.from == "" {
				session.from = "-"
			}
			fmt.Fprintf(output, "%sfrom %s to %s [%s >> %s]\n", prefix, session.from, m[2], session.inbound, defaultTag)
			continue
		}
		if strings.Contains(line, "[Info]") {
			continue
		}
		fmt.Fprintln(output, line)
	}
}

func pagerCommands(mode string) []*exec.Cmd {
	custom := strings.Fields(os.Getenv("PAGER"))
	if len(custom) == 0 {
		return []*exec.Cmd{exec.Command("less", mode)}
	}
	if custom[0] == "less" {
		for _, arg := range custom[1:] {
			if strings.HasPrefix(arg, "+") {
				return []*exec.Cmd{exec.Command(custom[0], custom[1:]...)}
			}
		}
		args := append(append([]string{}, custom[1:]...), mode)
		return []*exec.Cmd{exec.Command(custom[0], args...)}
	}
	return []*exec.Cmd{exec.Command(custom[0], custom[1:]...), exec.Command("less", mode)}
}

func runPager(source io.Reader, cmds []*exec.Cmd) error {
	for i, command := range cmds {
		command.Stderr = os.Stderr
		if i == len(cmds)-1 {
			command.Stdin = source
			command.Stdout = os.Stdout
			continue
		}
		reader, writer, err := os.Pipe()
		if err != nil {
			return err
		}
		command.Stdin = source
		command.Stdout = writer
		command.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
		source = reader
		if err := command.Start(); err != nil {
			return err
		}
		writer.Close()
	}
	err := cmds[len(cmds)-1].Run()
	for _, command := range cmds[:len(cmds)-1] {
		command.Wait()
	}
	return err
}

func newLogsCommand() *cobra.Command {
	var lines int
	var follow bool
	command := &cobra.Command{
		Use:   "log",
		Short: "Show the xray service log in a pager, pinned to the bottom",
		RunE: func(cmd *cobra.Command, args []string) error {
			if _, err := exec.LookPath("journalctl"); err != nil {
				return fmt.Errorf("journalctl not found")
			}
			stat, _ := os.Stdin.Stat()
			interactive := stat != nil && stat.Mode()&os.ModeCharDevice != 0
			base := []string{"-u", "xray", "--no-pager", "-n", strconv.Itoa(lines), "-o", "cat"}
			defaultTag := "default"
			if state, err := Load(); err == nil && state.Active != "" {
				defaultTag = state.Active
			}
			if follow && interactive {
				argv := journalArgv(base, true)
				jr, jw, err := os.Pipe()
				if err != nil {
					return err
				}
				pr, pw, err := os.Pipe()
				if err != nil {
					return err
				}
				journal := exec.Command(argv[0], argv[1:]...)
				journal.Stdout = jw
				journal.Stderr = os.Stderr
				journal.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
				if err := journal.Start(); err != nil {
					return err
				}
				jw.Close()
				go filterLogLines(jr, pw, defaultTag)
				pagerErr := runPager(pr, pagerCommands("+F"))
				journal.Process.Kill()
				journal.Wait()
				pw.Close()
				jr.Close()
				pr.Close()
				return pagerErr
			}
			if follow {
				argv := journalArgv(base, true)
				jr, jw, err := os.Pipe()
				if err != nil {
					return err
				}
				stream := exec.Command(argv[0], argv[1:]...)
				stream.Stdin = os.Stdin
				stream.Stdout = jw
				stream.Stderr = os.Stderr
				if err := stream.Start(); err != nil {
					return err
				}
				jw.Close()
				filterLogLines(jr, os.Stdout, defaultTag)
				jr.Close()
				return stream.Wait()
			}
			argv := journalArgv(base, false)
			journal := exec.Command(argv[0], argv[1:]...)
			var stderr bytes.Buffer
			journal.Stderr = &stderr
			data, err := journal.Output()
			if err != nil {
				if argv[0] == "sudo" {
					return fmt.Errorf("failed to read the xray journal: %s", strings.TrimSpace(stderr.String()))
				}
				elevated := exec.Command("sudo", argv...)
				elevated.Stdin = os.Stdin
				elevated.Stderr = os.Stderr
				data, err = elevated.Output()
				if err != nil {
					return fmt.Errorf("failed to read the xray journal: %s", strings.TrimSpace(stderr.String()))
				}
			}
			var filtered bytes.Buffer
			filterLogLines(bytes.NewReader(data), &filtered, defaultTag)
			if !interactive {
				os.Stdout.Write(filtered.Bytes())
				return nil
			}
			return runPager(bytes.NewReader(filtered.Bytes()), pagerCommands("+G"))
		},
	}
	command.Flags().IntVarP(&lines, "lines", "n", 500, "number of journal lines to read")
	command.Flags().BoolVarP(&follow, "follow", "f", false, "keep streaming new lines")
	return command
}

func journalArgv(base []string, follow bool) []string {
	argv := append([]string{"journalctl"}, base...)
	if follow {
		argv = append(argv, "-f")
	}
	if exec.Command("journalctl", "-u", "xray", "-n", "0", "--no-pager").Run() != nil {
		return append([]string{"sudo"}, argv...)
	}
	return argv
}

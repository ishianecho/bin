use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use clap_complete::CompleteEnv;

use super::{config, env, ssh};

#[derive(Parser)]
#[command(name = "proxy", version, about = "Manage and apply a proxy configuration")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Save and apply a proxy configuration")]
    Set {
        #[arg(long, help = "HTTP/HTTPS proxy, for example http://127.0.0.1:10808")]
        http: Option<String>,
        #[arg(long, help = "SOCKS proxy, for example socks5://127.0.0.1:10808")]
        socks: Option<String>,
        #[arg(long, help = "comma-separated hosts which bypass the proxy")]
        no_proxy: Option<String>,
        #[arg(value_name = "HTTP_PROXY")]
        http_url: Option<String>,
        #[arg(value_name = "SOCKS_PROXY")]
        socks_url: Option<String>,
    },
    #[command(about = "Connect through the configured SOCKS/HTTP proxy")]
    Ssh {
        #[arg(value_name = "HOST")]
        host: String,
        #[arg(value_name = "PORT")]
        port: String,
    },
    #[command(about = "Remove all proxy settings")]
    Clear,
    #[command(about = "Show the saved proxy configuration")]
    Show,
    #[command(about = "Re-apply the saved configuration to the environment")]
    Sync,
    #[command(about = "Generate the autocompletion script for the specified shell")]
    Completion {
        shell: CompletionShell,
    },
}

#[derive(clap::ValueEnum, Clone)]
enum CompletionShell {
    Bash,
    Fish,
    Powershell,
    Zsh,
}

fn command_factory() -> clap::Command {
    Cli::command()
}

fn long_about() -> String {
    format!(
        "Manage a proxy configuration in {} and apply it\nthrough environment variables. Shell sessions and ssh pick the\nfile up directly; no per-tool configuration is written.",
        config::config_path().display()
    )
}

pub fn run() -> Result<(), String> {
    CompleteEnv::with_factory(command_factory).complete();
    let mut command = Cli::command().long_about(long_about());
    let mut matches = command.clone().get_matches();
    let cli = Cli::from_arg_matches_mut(&mut matches)
        .map_err(|e| e.render().to_string())?;
    match cli.command {
        Some(Command::Set {
            http,
            socks,
            no_proxy,
            http_url,
            socks_url,
        }) => {
            let mut http = http.unwrap_or_default();
            let mut socks = socks.unwrap_or_default();
            let no_proxy = no_proxy.unwrap_or_default();
            for arg in [http_url, socks_url].into_iter().flatten() {
                if http.is_empty() {
                    http = arg;
                } else if socks.is_empty() {
                    socks = arg;
                }
            }
            if http.is_empty() && socks.is_empty() {
                return Err("set requires --http or --socks".into());
            }
            let mut cfg = config::Config {
                http,
                socks,
                no_proxy,
            };
            if let Ok(saved) = config::read() {
                if cfg.http.is_empty() {
                    cfg.http = saved.http;
                }
                if cfg.socks.is_empty() {
                    cfg.socks = saved.socks;
                }
                if cfg.no_proxy.is_empty() {
                    cfg.no_proxy = saved.no_proxy;
                }
            }
            config::validate(&cfg.http, "HTTP_PROXY")?;
            config::validate(&cfg.socks, "SOCKS_PROXY")?;
            if cfg.no_proxy.is_empty() {
                cfg.no_proxy = "localhost,127.0.0.1,::1".into();
            }
            config::write(&cfg)?;
            let applied = config::read()?;
            env::apply(&applied)?;
            println!("Proxy configuration: {}", config::config_path().display());
        }
        Some(Command::Ssh { host, port }) => ssh::relay(&host, &port)?,
        Some(Command::Clear) => {
            config::write(&config::Config::default())?;
            env::apply(&config::Config::default())?;
            println!("Proxy configuration: {}", config::config_path().display());
        }
        Some(Command::Show) => {
            let data = std::fs::read_to_string(config::config_path())
                .map_err(|_| format!("No proxy configuration found at {}", config::config_path().display()))?;
            print!("{data}");
        }
        Some(Command::Sync) => {
            let cfg = config::read()?;
            env::apply(&cfg)?;
            println!("Proxy configuration: {}", config::config_path().display());
        }
        Some(Command::Completion { shell }) => {
            let value = match shell {
                CompletionShell::Bash => "bash",
                CompletionShell::Fish => "fish",
                CompletionShell::Powershell => "powershell",
                CompletionShell::Zsh => "zsh",
            };
            std::env::set_var("COMPLETE", value);
            CompleteEnv::with_factory(command_factory).complete();
        }
        None => {
            command.print_long_help().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

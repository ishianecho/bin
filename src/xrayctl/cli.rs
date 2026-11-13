use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use clap_complete::engine::ArgValueCandidates;
use clap_complete::CompleteEnv;

use super::{
    completions,
    regen::{look_path, regenerate, sync_proxy_env},
    state::{self, Node, Ports, Route},
    vless::{
        encode_vless_link, parse_vless_link, validate_matcher, validate_node, validate_port,
    },
    service_action, service_state, sync_tproxy_env,
};

#[derive(Parser)]
#[command(name = "xrayctl", version, about = "State-driven manager for a local Xray service")]
struct Cli {
    #[arg(long, global = true, help = "skip the service restart after a mutating command")]
    no_restart: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Manage nodes")]
    Node {
        #[command(subcommand)]
        command: NodeCommand,
    },
    #[command(about = "Manage routing rules (evaluated before all built-in rules)")]
    Route {
        #[command(subcommand)]
        command: RouteCommand,
    },
    #[command(about = "Manage listening ports")]
    Port {
        #[command(subcommand)]
        command: PortCommand,
    },
    #[command(about = "Show service state, active node, ports, and route count")]
    Status,
    #[command(about = "Start the xray service")]
    Start,
    #[command(about = "Stop the xray service")]
    Stop,
    #[command(about = "Regenerate, validate, and restart the service")]
    Restart,
    #[command(about = "Validate the generated configuration")]
    Test,
    #[command(about = "First-run: import the env file, seed defaults")]
    Init,
    #[cfg(target_os = "linux")]
    #[command(about = "Transparent-proxy commands")]
    Tproxy {
        #[command(subcommand)]
        command: TproxyCommand,
    },
    #[cfg(target_os = "linux")]
    #[command(about = "Run a command outside the transparent proxy")]
    Direct {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, value_name = "CMD")]
        args: Vec<String>,
    },
    #[cfg(target_os = "linux")]
    #[command(about = "Show the xray service log in a pager, pinned to the bottom")]
    Log {
        #[arg(short = 'n', long, default_value_t = 500, help = "number of journal lines to read")]
        lines: i64,
        #[arg(short = 'f', long, help = "keep streaming new lines")]
        follow: bool,
    },
    #[cfg(windows)]
    #[command(about = "Manage the xray service (NSSM; requires an elevated terminal)")]
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },
    #[command(about = "Generate the autocompletion script for the specified shell")]
    Completion {
        shell: CompletionShell,
    },
}

#[derive(Subcommand)]
enum NodeCommand {
    #[command(about = "Add a vless+reality node; when the link carries a #tag the tag argument may be omitted")]
    Add {
        #[arg(long, help = "server address")]
        address: Option<String>,
        #[arg(long, help = "server port")]
        port: Option<i64>,
        #[arg(long, help = "user UUID")]
        uuid: Option<String>,
        #[arg(long, help = "reality server name")]
        sni: Option<String>,
        #[arg(long, help = "reality public key")]
        public_key: Option<String>,
        #[arg(long, help = "reality short ID")]
        short_id: Option<String>,
        #[arg(long, help = "make the new node active")]
        use_: bool,
        #[arg(value_name = "ARG")]
        args: Vec<String>,
    },
    #[command(about = "Remove nodes")]
    Rm {
        #[arg(required = true, value_name = "TAG", add = ArgValueCandidates::new(completions::node_tags))]
        tags: Vec<String>,
    },
    #[command(about = "Rename a node")]
    Rn {
        #[arg(value_name = "OLD", add = ArgValueCandidates::new(completions::node_tags))]
        old: String,
        #[arg(value_name = "NEW")]
        new: String,
    },
    #[command(about = "List nodes (* marks the active node)")]
    List,
    #[command(about = "Set the active node")]
    Use {
        #[arg(value_name = "TAG", add = ArgValueCandidates::new(completions::node_tags))]
        tag: String,
    },
    #[command(about = "Show node details")]
    Show {
        #[arg(value_name = "TAG", add = ArgValueCandidates::new(completions::node_tags))]
        tag: Option<String>,
        #[arg(long, help = "print the vless:// link instead of JSON")]
        link: bool,
    },
    #[command(about = "Update a node; keys: address, port, uuid, sni, public-key, short-id")]
    Set {
        #[arg(value_name = "TAG", add = ArgValueCandidates::new(completions::node_tags))]
        tag: String,
        #[arg(value_name = "KEY_VALUE", add = ArgValueCandidates::new(completions::node_keys))]
        pairs: Vec<String>,
        #[arg(long, help = "server address")]
        address: Option<String>,
        #[arg(long, help = "server port")]
        port: Option<i64>,
        #[arg(long, help = "user UUID")]
        uuid: Option<String>,
        #[arg(long, help = "reality server name")]
        sni: Option<String>,
        #[arg(long, help = "reality public key")]
        public_key: Option<String>,
        #[arg(long, help = "reality short ID")]
        short_id: Option<String>,
    },
}

#[derive(Subcommand)]
enum RouteCommand {
    #[command(about = "Add routing rules; matcher: domain:xxx.com, geosite:cn, full:example.com, regexp:pat, geoip:cn, or a bare IP/CIDR")]
    Add {
        #[arg(value_name = "ACTION", add = ArgValueCandidates::new(completions::route_actions))]
        action: String,
        #[arg(required = true, value_name = "MATCHER", add = ArgValueCandidates::new(completions::matcher_prefixes))]
        matchers: Vec<String>,
    },
    #[command(about = "Remove routing rules")]
    Rm {
        #[arg(required = true, value_name = "MATCHER", add = ArgValueCandidates::new(completions::route_matchers))]
        matchers: Vec<String>,
    },
    #[command(about = "List routing rules")]
    List,
}

#[derive(Subcommand)]
enum PortCommand {
    #[command(about = "Set the socks or tproxy port")]
    Set {
        #[arg(long, help = "socks port")]
        socks: Option<i64>,
        #[arg(long, help = "tproxy port")]
        tproxy: Option<i64>,
    },
}

#[cfg(target_os = "linux")]
#[derive(Subcommand)]
enum TproxyCommand {
    #[command(about = "Show transparent-proxy rule state")]
    Status,
}

#[cfg(windows)]
#[derive(Subcommand)]
enum ServiceCommand {
    #[command(about = "install the xray service")]
    Install,
    #[command(about = "uninstall the xray service")]
    Uninstall,
    #[command(about = "start the xray service")]
    Start,
    #[command(about = "stop the xray service")]
    Stop,
    #[command(about = "restart the xray service")]
    Restart,
    #[command(about = "status the xray service")]
    Status,
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
        "State-driven manager for a local Xray service (vless + reality).\nState lives in {}; {} is\ngenerated output. Do not edit config.json by hand.",
        state::state_path().display(),
        state::config_path().display()
    )
}

pub fn run() -> Result<(), String> {
    CompleteEnv::with_factory(command_factory).complete();
    let mut command = Cli::command().long_about(long_about());
    let mut matches = command.clone().get_matches();
    let cli = Cli::from_arg_matches_mut(&mut matches).map_err(|e| e.render().to_string())?;
    match cli.command {
        Some(Commands::Node { command }) => run_node(command, cli.no_restart)?,
        Some(Commands::Route { command }) => run_route(command, cli.no_restart)?,
        Some(Commands::Port { command: PortCommand::Set { socks, tproxy } }) => {
            run_port_set(socks, tproxy, cli.no_restart)?
        }
        Some(Commands::Status) => {
            let state = state::load()?;
            println!("service: {}", service_state());
            println!("node: {}", state.active);
            println!("ports: socks={} tproxy={}", state.ports.socks, state.ports.tproxy);
            println!("routes: {} custom", state.routes.len());
        }
        Some(Commands::Start) => service_action("start")?,
        Some(Commands::Stop) => service_action("stop")?,
        Some(Commands::Restart) => {
            let state = state::load()?;
            regenerate(&state, cli.no_restart)?;
        }
        Some(Commands::Test) => run_test()?,
        Some(Commands::Init) => run_init(cli.no_restart)?,
        #[cfg(target_os = "linux")]
        Some(Commands::Tproxy { command: TproxyCommand::Status }) => super::service_linux::tproxy_status(),
        #[cfg(target_os = "linux")]
        Some(Commands::Direct { args }) => super::service_linux::direct(&args)?,
        #[cfg(target_os = "linux")]
        Some(Commands::Log { lines, follow }) => super::logs_linux::run_log(lines, follow)?,
        #[cfg(windows)]
        Some(Commands::Service { command }) => run_service(command)?,
        Some(Commands::Completion { shell }) => {
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

fn run_node(command: NodeCommand, no_restart: bool) -> Result<(), String> {
    match command {
        NodeCommand::Add {
            address,
            port,
            uuid,
            sni,
            public_key,
            short_id,
            use_,
            args,
        } => {
            if args.len() > 2 {
                return Err(format!("Unexpected argument: {}", args[2]));
            }
            let mut tag = String::new();
            let mut link = String::new();
            for arg in args {
                if arg.starts_with("vless://") {
                    if !link.is_empty() {
                        return Err("Only one vless:// link is allowed".into());
                    }
                    link = arg;
                } else if tag.is_empty() {
                    tag = arg;
                } else {
                    return Err(format!("Unexpected argument: {arg}"));
                }
            }
            let mut node = if !link.is_empty() {
                let mut node = parse_vless_link(&link)?;
                if tag.is_empty() {
                    tag = node.tag.clone();
                }
                if tag.is_empty() {
                    return Err("The link has no #tag; provide a tag explicitly".into());
                }
                node.tag = tag;
                node
            } else {
                Node {
                    tag,
                    address: address.unwrap_or_default(),
                    port: port.unwrap_or(0),
                    uuid: uuid.unwrap_or_default(),
                    sni: sni.unwrap_or_default(),
                    public_key: public_key.unwrap_or_default(),
                    short_id: short_id.unwrap_or_default(),
                    security: String::new(),
                    pin: String::new(),
                }
            };
            let name = format!("node '{}'", node.tag);
            validate_node(&mut node, &name)?;
            let mut state = state::load()?;
            if state.find_node(&node.tag).is_some() {
                return Err(format!(
                    "Node '{}' already exists; use: xrayctl node set",
                    node.tag
                ));
            }
            let was_empty = state.active.is_empty();
            let tag = node.tag.clone();
            state.nodes.push(node);
            if was_empty || use_ {
                state.active = tag.clone();
            }
            state::save(&state)?;
            println!("Added node {tag}");
            regenerate(&state, no_restart)
        }
        NodeCommand::Rm { tags } => {
            let mut state = state::load()?;
            for tag in &tags {
                if state.find_node(tag).is_none() {
                    return Err(format!("Node '{tag}' not found"));
                }
            }
            let count = tags.len();
            state.nodes.retain(|node| !tags.contains(&node.tag));
            if tags.contains(&state.active) {
                state.active = String::new();
            }
            state::save(&state)?;
            println!("Removed {count} node(s)");
            if !state.active.is_empty() {
                return regenerate(&state, no_restart);
            }
            eprintln!("No active node; run: xrayctl node use <tag>");
            Ok(())
        }
        NodeCommand::Rn { old, new } => {
            let mut state = state::load()?;
            if state.find_node(&old).is_none() {
                return Err(format!("Node '{old}' not found"));
            }
            if state.find_node(&new).is_some() {
                return Err(format!("Node '{new}' already exists"));
            }
            if let Some(node) = state.find_node(&old) {
                node.tag = new.clone();
            }
            let mut active_changed = false;
            if state.active == old {
                state.active = new.clone();
                active_changed = true;
            }
            state::save(&state)?;
            println!("Renamed node {old} to {new}");
            if active_changed && state.active == new {
                return regenerate(&state, no_restart);
            }
            Ok(())
        }
        NodeCommand::List => {
            let state = state::load()?;
            for node in &state.nodes {
                let marker = if state.active == node.tag { " *" } else { "" };
                println!("{}{}\t{}:{}", node.tag, marker, node.address, node.port);
            }
            Ok(())
        }
        NodeCommand::Use { tag } => {
            let mut state = state::load()?;
            if state.find_node(&tag).is_none() {
                return Err(format!("Node '{tag}' not found"));
            }
            state.active = tag.clone();
            state::save(&state)?;
            println!("Active node: {tag}");
            regenerate(&state, no_restart)
        }
        NodeCommand::Show { tag, link } => {
            let state = state::load()?;
            let tag = match tag {
                Some(tag) => tag,
                None => state.active.clone(),
            };
            if tag.is_empty() {
                return Err("No active node; run: xrayctl node use <tag>".into());
            }
            let node = state
                .nodes
                .iter()
                .find(|node| node.tag == tag)
                .ok_or_else(|| format!("Node '{tag}' not found"))?;
            if link {
                println!("{}", encode_vless_link(node));
            } else {
                let mut data = serde_json::to_string_pretty(node).map_err(|e| e.to_string())?;
                data.push('\n');
                print!("{data}");
            }
            Ok(())
        }
        NodeCommand::Set {
            tag,
            pairs,
            address,
            port,
            uuid,
            sni,
            public_key,
            short_id,
        } => {
            if pairs.len() % 2 != 0 {
                return Err("Expected key value pairs after the tag".into());
            }
            let mut state = state::load()?;
            if state.find_node(&tag).is_none() {
                return Err(format!("Node '{tag}' not found"));
            }
            let node = state.find_node(&tag).unwrap();
            let mut i = 0;
            while i < pairs.len() {
                let (key, value) = (&pairs[i], &pairs[i + 1]);
                if key != "port" && value.contains([' ', '\t', '\n', '\r', '"', '\'', '\\']) {
                    return Err("Values must not contain whitespace, quotes, or backslashes".into());
                }
                match key.as_str() {
                    "address" => node.address = value.clone(),
                    "uuid" => node.uuid = value.clone(),
                    "sni" => node.sni = value.clone(),
                    "public-key" => node.public_key = value.clone(),
                    "short-id" => node.short_id = value.clone(),
                    "port" => {
                        let port = value
                            .parse::<i64>()
                            .map_err(|_| "port must be a number".to_string())?;
                        validate_port(port)?;
                        node.port = port;
                    }
                    _ => return Err(format!("Unknown key: {key}")),
                }
                i += 2;
            }
            for (value, target) in [
                (address, &mut node.address),
                (uuid, &mut node.uuid),
                (sni, &mut node.sni),
                (public_key, &mut node.public_key),
                (short_id, &mut node.short_id),
            ] {
                if let Some(value) = value {
                    if value.contains([' ', '\t', '\n', '\r', '"', '\'', '\\']) {
                        return Err(
                            "Values must not contain whitespace, quotes, or backslashes".into()
                        );
                    }
                    *target = value;
                }
            }
            if let Some(port) = port {
                validate_port(port)?;
                node.port = port;
            }
            state::save(&state)?;
            println!("Updated node {tag}");
            if state.active == tag {
                return regenerate(&state, no_restart);
            }
            Ok(())
        }
    }
}

fn run_route(command: RouteCommand, no_restart: bool) -> Result<(), String> {
    match command {
        RouteCommand::Add { action, matchers } => {
            if action != "direct" && action != "proxy" && action != "block" {
                return Err("Action must be direct, proxy, or block".into());
            }
            for matcher in &matchers {
                validate_matcher(matcher)?;
            }
            let mut state = state::load()?;
            for matcher in &matchers {
                if state.find_route(matcher).is_some() {
                    return Err(format!(
                        "Route '{matcher}' already exists; run: xrayctl route rm {matcher} first"
                    ));
                }
            }
            let count = matchers.len();
            for matcher in matchers {
                state.routes.push(Route {
                    action: action.clone(),
                    matcher,
                });
            }
            state::save(&state)?;
            println!("Added {count} route(s)");
            regenerate(&state, no_restart)
        }
        RouteCommand::Rm { matchers } => {
            let mut state = state::load()?;
            for matcher in &matchers {
                if state.find_route(matcher).is_none() {
                    return Err(format!("Route not found: {matcher}"));
                }
            }
            let count = matchers.len();
            state.routes.retain(|route| !matchers.contains(&route.matcher));
            state::save(&state)?;
            println!("Removed {count} route(s)");
            regenerate(&state, no_restart)
        }
        RouteCommand::List => {
            let state = state::load()?;
            for route in &state.routes {
                println!("{} {}", route.action, route.matcher);
            }
            Ok(())
        }
    }
}

fn run_port_set(socks: Option<i64>, tproxy: Option<i64>, no_restart: bool) -> Result<(), String> {
    if socks.is_none() && tproxy.is_none() {
        return Err("port set requires --socks or --tproxy".into());
    }
    let mut state = state::load()?;
    let current_socks = state.ports.socks;
    let current_tproxy = state.ports.tproxy;
    let socks = socks.unwrap_or(current_socks);
    let tproxy = tproxy.unwrap_or(current_tproxy);
    if tproxy != current_tproxy && cfg!(windows) {
        println!("The tproxy port has no effect on Windows");
    }
    validate_port(socks)?;
    validate_port(tproxy)?;
    if socks == tproxy {
        return Err("The socks and tproxy ports must differ".into());
    }
    state.ports = Ports { socks, tproxy };
    state::save(&state)?;
    sync_tproxy_env(tproxy, tproxy != current_tproxy)?;
    println!("Ports: socks={socks} tproxy={tproxy}");
    regenerate(&state, no_restart)?;
    if socks != current_socks {
        sync_proxy_env(socks);
    }
    Ok(())
}

fn run_test() -> Result<(), String> {
    if !state::config_path().exists() {
        return Err(format!(
            "No generated config at {}; run: xrayctl init",
            state::config_path().display()
        ));
    }
    if look_path("xray").is_none() {
        return Err("xray not found".into());
    }
    let status = std::process::Command::new("xray")
        .args(["run", "-test", "-c"])
        .arg(state::config_path())
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("xray run -test failed".into());
    }
    Ok(())
}

fn run_init(no_restart: bool) -> Result<(), String> {
    if !state::state_path().exists() {
        if state::env_path().exists() {
            return import_env();
        }
        state::load()?;
        println!("Created empty state at {}", state::state_path().display());
        println!("Add a node: xrayctl node add <tag> vless://...");
        return Ok(());
    }
    let state = state::load()?;
    regenerate(&state, no_restart)
}

fn import_env() -> Result<(), String> {
    let data = std::fs::read_to_string(state::env_path()).map_err(|e| e.to_string())?;
    let mut values = std::collections::BTreeMap::new();
    for line in data.split('\n') {
        if let Some(index) = line.find('=') {
            if index > 0 && line.starts_with("XRAY_") {
                values.insert(line[..index].to_string(), line[index + 1..].to_string());
            }
        }
    }
    if values.get("XRAY_ADDRESS").map(String::is_empty).unwrap_or(true)
        || values.get("XRAY_UUID").map(String::is_empty).unwrap_or(true)
    {
        return Err(format!(
            "{} lacks XRAY_ADDRESS or XRAY_UUID",
            state::env_path().display()
        ));
    }
    let mut port = 443;
    if let Some(value) = values.get("XRAY_PORT") {
        if !value.is_empty() {
            port = value
                .parse::<i64>()
                .map_err(|_| "env node has an invalid port".to_string())?;
        }
    }
    let get = |key: &str| values.get(key).cloned().unwrap_or_default();
    let mut node = Node {
        tag: get("XRAY_TAG"),
        address: get("XRAY_ADDRESS"),
        port,
        uuid: get("XRAY_UUID"),
        sni: get("XRAY_SNI"),
        public_key: get("XRAY_PUBLIC_KEY"),
        short_id: get("XRAY_SHORT_ID"),
        security: String::new(),
        pin: String::new(),
    };
    validate_node(&mut node, "env node")?;
    let mut state = state::load()?;
    state.nodes = vec![node.clone()];
    state.active = node.tag.clone();
    state.routes = vec![
        Route {
            action: "direct".into(),
            matcher: "domain:sourceforge.net".into(),
        },
        Route {
            action: "direct".into(),
            matcher: "domain:stackoverflow.com".into(),
        },
    ];
    state::save(&state)?;
    println!("Imported node {} from {}", node.tag, state::env_path().display());
    println!(
        "The env file is deprecated; state.json is authoritative and {} may be deleted",
        state::env_path().display()
    );
    Ok(())
}

#[cfg(windows)]
fn run_service(command: ServiceCommand) -> Result<(), String> {
    use super::service_windows::{install_service, require_admin, uninstall_service};
    match command {
        ServiceCommand::Install => {
            require_admin()?;
            install_service()
        }
        ServiceCommand::Uninstall => {
            require_admin()?;
            uninstall_service()
        }
        ServiceCommand::Start => {
            require_admin()?;
            run_nssm_action("start")
        }
        ServiceCommand::Stop => {
            require_admin()?;
            run_nssm_action("stop")
        }
        ServiceCommand::Restart => {
            require_admin()?;
            run_nssm_action("restart")
        }
        ServiceCommand::Status => {
            println!("{}", service_state());
            Ok(())
        }
    }
}

#[cfg(windows)]
fn run_nssm_action(action: &str) -> Result<(), String> {
    use super::service_windows::{nssm_path, run_nssm};
    let nssm = nssm_path()?;
    run_nssm(&nssm, &[action])
}

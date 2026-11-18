use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use clap_complete::engine::ArgValueCandidates;
use clap_complete::CompleteEnv;

use crate::xrayctl::{
    regen::{clear_proxy_env, look_path, sync_proxy_env},
    ssh,
    vless::{encode_vless_link, parse_vless_link, validate_matcher, validate_node, validate_port},
};

use super::{
    completions,
    mesh,
    regen::regenerate,
    service_action, service_state,
    state::{self, Node, Route, State},
};

#[derive(Parser)]
#[command(name = "sbctl", version, about = "State-driven manager for a local sing-box service")]
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
    #[command(about = "Manage the wireguard mesh")]
    Mesh {
        #[command(subcommand)]
        command: MeshCommand,
    },
    #[command(about = "Show service state, active node, ports, mesh, and route count")]
    Status,
    #[command(about = "Start the sing-box service")]
    Start,
    #[command(about = "Stop the sing-box service")]
    Stop,
    #[command(about = "Regenerate, validate, and restart the service")]
    Restart,
    #[command(about = "Validate the generated configuration")]
    Test,
    #[command(about = "First-run: import nodes and routes from the xray state")]
    Init,
    #[cfg(target_os = "linux")]
    #[command(about = "Show the sing-box service log")]
    Log {
        #[arg(short = 'n', long, default_value_t = 500, help = "number of journal lines to read")]
        lines: i64,
        #[arg(short = 'f', long, help = "keep streaming new lines")]
        follow: bool,
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
    #[command(about = "Pull a subscription URL and upsert nodes by tag (vless:// links, plain or base64)")]
    Pull {
        #[arg(value_name = "URL")]
        url: String,
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
    #[command(about = "Set the mixed port")]
    Set {
        #[arg(long, help = "mixed (http+socks) port")]
        mixed: i64,
    },
}

#[derive(Subcommand)]
enum MeshCommand {
    #[command(about = "Import the wireguard endpoint from a vpn-repo generated sing-box config")]
    Import {
        #[arg(value_name = "PATH")]
        path: String,
    },
    #[command(about = "Enable the wireguard mesh")]
    On,
    #[command(about = "Disable the wireguard mesh")]
    Off,
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
        "State-driven manager for a local sing-box service (tun + vless + reality).\nState lives in {}; {} is\ngenerated output and installed to /etc/sing-box/config.json. Do not edit them by hand.",
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
        Some(Commands::Port { command: PortCommand::Set { mixed } }) => {
            run_port_set(mixed, cli.no_restart)?
        }
        Some(Commands::Mesh { command }) => run_mesh(command, cli.no_restart)?,
        Some(Commands::Status) => {
            let state = state::load()?;
            println!("service: {}", service_state());
            println!("node: {}", state.active);
            println!("mixed: {}", state.mixed_port);
            match state.mesh {
                Some(mesh) => println!("mesh: {} ({})", if mesh.enabled { "on" } else { "off" }, mesh.address),
                None => println!("mesh: -"),
            }
            println!("routes: {} custom", state.routes.len());
        }
        Some(Commands::Start) => {
            let state = state::load()?;
            regenerate(&state, true)?;
            service_action("start")?;
            sync_proxy_env(state.mixed_port);
            ssh::sync(true);
        }
        Some(Commands::Stop) => {
            service_action("stop")?;
            clear_proxy_env();
            ssh::sync(false);
        }
        Some(Commands::Restart) => {
            let state = state::load()?;
            regenerate(&state, cli.no_restart)?;
        }
        Some(Commands::Test) => run_test()?,
        Some(Commands::Init) => run_init(cli.no_restart)?,        #[cfg(target_os = "linux")]
        Some(Commands::Log { lines, follow }) => super::logs_linux::run_log(lines, follow)?,
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
        NodeCommand::Pull { url } => run_node_pull(&url, no_restart),
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
            if !node.pin.is_empty() {
                eprintln!("Note: sing-box has no equivalent for certificate pins; the pin is ignored");
            }
            let mut state = state::load()?;
            if state.find_node(&node.tag).is_some() {
                return Err(format!(
                    "Node '{}' already exists; use: sbctl node set",
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
            eprintln!("No active node; run: sbctl node use <tag>");
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
                return Err("No active node; run: sbctl node use <tag>".into());
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

fn fetch_subscription(url: &str) -> Result<String, String> {
    if look_path("curl").is_none() {
        return Err("curl not found".into());
    }
    let output = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "30"])
        .arg(url)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("curl exited with {}", output.status));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| "subscription body is not valid UTF-8".to_string())
}

fn extract_links(body: &str) -> Vec<String> {
    let mut links = filter_links(body);
    if links.is_empty() {
        use base64::{engine::general_purpose::{STANDARD, STANDARD_NO_PAD}, Engine as _};
        let trimmed = body.trim();
        let decoded = STANDARD
            .decode(trimmed)
            .or_else(|_| STANDARD_NO_PAD.decode(trimmed));
        if let Ok(data) = decoded {
            if let Ok(text) = String::from_utf8(data) {
                links = filter_links(&text);
            }
        }
    }
    links
}

fn filter_links(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("vless://"))
        .map(str::to_string)
        .collect()
}

fn run_node_pull(url: &str, no_restart: bool) -> Result<(), String> {
    let body = fetch_subscription(url)?;
    let links = extract_links(&body);
    if links.is_empty() {
        return Err("No vless:// links found in the subscription".into());
    }
    let mut state = state::load()?;
    let was_empty = state.active.is_empty();
    let mut added = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    let mut pinned = false;
    let mut changed_tags: Vec<String> = Vec::new();
    for link in &links {
        let mut node = parse_vless_link(link).map_err(|e| format!("Invalid link: {e}"))?;
        if node.tag.is_empty() {
            return Err("A link in the subscription has no #tag".into());
        }
        let name = format!("node '{}'", node.tag);
        validate_node(&mut node, &name)?;
        pinned = pinned || !node.pin.is_empty();
        let tag = node.tag.clone();
        match state.nodes.iter().position(|n| n.tag == tag) {
            Some(index) => {
                if state.nodes[index] == node {
                    unchanged += 1;
                } else {
                    state.nodes[index] = node;
                    updated += 1;
                    changed_tags.push(tag);
                }
            }
            None => {
                state.nodes.push(node);
                added += 1;
                changed_tags.push(tag);
            }
        }
    }
    if was_empty {
        if let Some(node) = state.nodes.first() {
            state.active = node.tag.clone();
        }
    }
    let active_changed = was_empty || changed_tags.contains(&state.active);
    if pinned {
        eprintln!("Note: sing-box has no equivalent for certificate pins; the pin is ignored");
    }
    state::save(&state)?;
    if was_empty {
        println!("Active node: {}", state.active);
    }
    println!("Added {added}, updated {updated}, unchanged {unchanged}");
    if active_changed {
        return regenerate(&state, no_restart);
    }
    Ok(())
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
                        "Route '{matcher}' already exists; run: sbctl route rm {matcher} first"
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

fn run_port_set(mixed: i64, no_restart: bool) -> Result<(), String> {
    let mut state = state::load()?;
    let current = state.mixed_port;
    validate_port(mixed)?;
    state.mixed_port = mixed;
    state::save(&state)?;
    println!("Mixed port: {mixed}");
    regenerate(&state, no_restart)?;
    if mixed != current {
        sync_proxy_env(mixed);
    }
    Ok(())
}

fn run_mesh(command: MeshCommand, no_restart: bool) -> Result<(), String> {
    match command {
        MeshCommand::Import { path } => {
            let imported = mesh::import(&path)?;
            let mut state = state::load()?;
            let address = imported.address.clone();
            let subnet = imported.subnet.clone();
            state.mesh = Some(imported);
            state::save(&state)?;
            println!("Imported mesh {address} (subnet {subnet}); run: sbctl mesh on");
            Ok(())
        }
        MeshCommand::On => {
            let mut state = state::load()?;
            let mesh = state
                .mesh
                .as_mut()
                .ok_or("No mesh imported; run: sbctl mesh import <path>")?;
            mesh.enabled = true;
            let address = mesh.address.clone();
            state::save(&state)?;
            println!("Mesh on ({address})");
            regenerate(&state, no_restart)
        }
        MeshCommand::Off => {
            let mut state = state::load()?;
            let mesh = state
                .mesh
                .as_mut()
                .ok_or("No mesh imported; run: sbctl mesh import <path>")?;
            mesh.enabled = false;
            state::save(&state)?;
            println!("Mesh off");
            regenerate(&state, no_restart)
        }
    }
}

fn run_test() -> Result<(), String> {
    if !state::config_path().exists() {
        return Err(format!(
            "No generated config at {}; run: sbctl init",
            state::config_path().display()
        ));
    }
    if look_path("sing-box").is_none() {
        return Err("sing-box not found".into());
    }
    let status = std::process::Command::new("sing-box")
        .args(["check", "-c"])
        .arg(state::config_path())
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("sing-box check failed".into());
    }
    Ok(())
}

fn run_init(no_restart: bool) -> Result<(), String> {
    if state::state_path().exists() {
        let state = state::load()?;
        return regenerate(&state, no_restart);
    }
    let xray_state_path = crate::xrayctl::state::state_path();
    if !xray_state_path.exists() {
        state::load()?;
        println!("Created empty state at {}", state::state_path().display());
        println!("Add a node: sbctl node add <tag> vless://...");
        return Ok(());
    }
    let xray = crate::xrayctl::state::load()?;
    if xray.nodes.is_empty() {
        state::load()?;
        println!("Created empty state at {}", state::state_path().display());
        println!("Add a node: sbctl node add <tag> vless://...");
        return Ok(());
    }
    if xray.nodes.iter().any(|node| !node.pin.is_empty()) {
        eprintln!("Note: sing-box has no equivalent for certificate pins; the pin is ignored");
    }
    let state = State {
        version: 1,
        active: xray.active.clone(),
        mixed_port: xray.ports.socks,
        nodes: xray.nodes.clone(),
        routes: xray.routes.clone(),
        mesh: None,
    };
    state::save(&state)?;
    println!(
        "Imported {} node(s) and {} route(s) from {}",
        state.nodes.len(),
        state.routes.len(),
        xray_state_path.display()
    );
    regenerate(&state, no_restart)
}

#[cfg(test)]
mod tests {
    use super::extract_links;
    use base64::Engine as _;

    #[test]
    fn extracts_plain_links() {
        let body = "vless://a@b:443?security=reality#one\nss://ignored\nvless://c@d:443#two\n";
        let links = extract_links(body);
        assert_eq!(links.len(), 2);
        assert!(links[0].ends_with("#one"));
    }

    #[test]
    fn extracts_base64_links() {
        let plain = "vless://a@b:443?security=reality#one\nvless://c@d:443#two\n";
        let body = base64::engine::general_purpose::STANDARD.encode(plain);
        let links = extract_links(&body);
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn extracts_unpadded_base64_links() {
        let plain = "vless://a@b:443?security=reality#one\n";
        let mut body = base64::engine::general_purpose::STANDARD.encode(plain);
        while body.ends_with('=') {
            body.pop();
        }
        let links = extract_links(&body);
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn extracts_nothing_from_garbage() {
        assert!(extract_links("hello world").is_empty());
    }
}

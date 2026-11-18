use clap_complete::engine::CompletionCandidate;

use super::state;

pub fn node_tags() -> Vec<CompletionCandidate> {
    state::load()
        .map(|state| {
            state
                .nodes
                .iter()
                .map(|node| CompletionCandidate::new(node.tag.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn route_matchers() -> Vec<CompletionCandidate> {
    state::load()
        .map(|state| {
            state
                .routes
                .iter()
                .map(|route| CompletionCandidate::new(route.matcher.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn route_actions() -> Vec<CompletionCandidate> {
    ["direct", "proxy", "block"]
        .into_iter()
        .map(CompletionCandidate::new)
        .collect()
}

pub fn matcher_prefixes() -> Vec<CompletionCandidate> {
    ["domain:", "geosite:", "full:", "regexp:", "geoip:"]
        .into_iter()
        .map(CompletionCandidate::new)
        .collect()
}

pub fn node_keys() -> Vec<CompletionCandidate> {
    ["address", "port", "uuid", "sni", "public-key", "short-id"]
        .into_iter()
        .map(CompletionCandidate::new)
        .collect()
}

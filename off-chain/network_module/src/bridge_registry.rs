use std::{env, fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

const DEFAULT_BRIDGE_PRIORITY: [&str; 3] = ["AXELAR", "LAYERZERO", "WORMHOLE"];
const DEFAULT_CONFIG_PATH: &str = "network_module/config/bridge_registry.json";

#[derive(Debug, Deserialize)]
struct BridgeEntry {
    name: String,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct BridgeRegistryConfig {
    bridges: Option<Vec<BridgeEntry>>,
    priority: Option<Vec<String>>,
}

pub struct BridgePrioritySelection {
    pub names: Vec<String>,
    pub source: String,
    pub raw: String,
}

fn normalize_name(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

fn parse_csv_priority(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(normalize_name)
        .filter(|name| !name.is_empty())
        .collect()
}

fn default_priority() -> Vec<String> {
    DEFAULT_BRIDGE_PRIORITY
        .iter()
        .map(|name| (*name).to_owned())
        .collect()
}

pub fn resolve_bridge_priority() -> Result<BridgePrioritySelection> {
    if let Some(raw) = env::var("BRIDGE_PRIORITY")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        let names = parse_csv_priority(&raw);
        return Ok(BridgePrioritySelection {
            names,
            source: "env:BRIDGE_PRIORITY".to_owned(),
            raw,
        });
    }

    let config_path = env::var("BRIDGE_REGISTRY_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_owned());

    if Path::new(&config_path).exists() {
        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("failed reading bridge registry config: {config_path}"))?;
        let config: BridgeRegistryConfig = serde_json::from_str(&content)
            .with_context(|| format!("invalid JSON bridge registry config: {config_path}"))?;

        let names = if let Some(priority) = config.priority {
            let parsed = priority
                .into_iter()
                .map(|value| normalize_name(&value))
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>();
            if parsed.is_empty() {
                default_priority()
            } else {
                parsed
            }
        } else if let Some(bridges) = config.bridges {
            let parsed = bridges
                .into_iter()
                .filter(|entry| entry.enabled.unwrap_or(true))
                .map(|entry| normalize_name(&entry.name))
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>();
            if parsed.is_empty() {
                default_priority()
            } else {
                parsed
            }
        } else {
            default_priority()
        };

        return Ok(BridgePrioritySelection {
            raw: names.join(","),
            names,
            source: format!("config:{config_path}"),
        });
    }

    let names = default_priority();
    Ok(BridgePrioritySelection {
        raw: names.join(","),
        names,
        source: "builtin-default".to_owned(),
    })
}

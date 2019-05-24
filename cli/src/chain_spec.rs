// Copyright 2018-2019 Chainpool.
use serde_json::json;

use telemetry::TelemetryEndpoints;

use chainx_runtime::GenesisConfig;

use super::genesis_config::{genesis, GenesisSpec};

const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const CHAINX_TELEMETRY_URL: &str = "wss://stats.chainx.org/submit/";

/// Specialised `ChainSpec`.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// Staging testnet config.
pub fn mainnet_config() -> ChainSpec {
    let boot_nodes = vec![
        "/ip4/47.96.134.203/tcp/31126/p2p/QmTZBuK6KCi5KXxJjsun5j6m46Gsj9BgSuo5MxaDfbGDJe".into(),
        "/ip4/47.96.97.52/tcp/31127/p2p/QmaiWDshcMMwEp5EbNKHhicqNQG6hWs6BquJqm3QTXgATW".into(),
        "/ip4/47.110.232.108/tcp/31129/p2p/QmZpqsZ5XMSMHThbiUFTUxCY3efjz2uGDGH3Jh3rAcKA8R".into(),
    ];
    ChainSpec::from_genesis(
        "ChainX Mainnet",
        "chainx_mainnet",
        mainnet_config_genesis,
        boot_nodes,
        Some(TelemetryEndpoints::new(vec![
            (STAGING_TELEMETRY_URL.to_string(), 0),
            (CHAINX_TELEMETRY_URL.to_string(), 0),
        ])),
        Some("ChainX Mainnet"),
        None,
        Some(
            json!({
                "network_type": "mainnet",
                "address_type": 44,
                "bitcoin_type": "mainnet"
            })
            .as_object()
            .unwrap()
            .to_owned(),
        ),
    )
}

fn mainnet_config_genesis() -> GenesisConfig {
    genesis(GenesisSpec::Mainnet)
}

fn development_config_genesis() -> GenesisConfig {
    genesis(GenesisSpec::Dev)
}

/// Development config (single validator Alice)
pub fn development_config() -> ChainSpec {
    ChainSpec::from_genesis(
        "Development",
        "dev",
        development_config_genesis,
        vec![],
        Some(TelemetryEndpoints::new(vec![(
            CHAINX_TELEMETRY_URL.to_string(),
            0,
        )])),
        Some("DEV ChainX V0.9.10"),
        None,
        Some(
            json!({
                "network_type": "testnet",
                "address_type": 44,
                "bitcoin_type": "mainnet"
            })
            .as_object()
            .unwrap()
            .to_owned(),
        ),
    )
}

fn testnet_genesis() -> GenesisConfig {
    genesis(GenesisSpec::Testnet)
}

pub fn testnet_config() -> ChainSpec {
    let boot_nodes = vec![];
    ChainSpec::from_genesis(
        "ChainX Local V0.9.10",
        "chainx_testnet",
        testnet_genesis,
        boot_nodes,
        Some(TelemetryEndpoints::new(vec![
            (STAGING_TELEMETRY_URL.to_string(), 0),
            (CHAINX_TELEMETRY_URL.to_string(), 0),
        ])),
        Some("ChainX Testnet V0.9.10"),
        None,
        Some(
            json!({
                "network_type": "testnet",
                "address_type": 44,
                "bitcoin_type": "mainnet"
            })
            .as_object()
            .unwrap()
            .to_owned(),
        ),
    )
}

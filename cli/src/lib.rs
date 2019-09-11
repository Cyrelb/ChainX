// Copyright 2018 Parity Technologies (UK) Ltd.
// Copyright 2018-2019 Chainpool.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Substrate CLI library.

#![feature(custom_attribute)]

mod chain_spec;
mod genesis_config;
mod native_rpc;
mod params;
mod service;

use std::ops::Deref;
use std::str::FromStr;

use log::LevelFilter;
use log::{info, warn};
use tokio::runtime::{Builder as RuntimeBuilder, Runtime};

pub use cli::{error, IntoExit, NoCustom, VersionInfo};
use substrate_service::{Roles as ServiceRoles, ServiceFactory};

use self::params::ChainXParams;
use self::service::set_validator_name;

/// The chain specification option.
#[derive(Clone, Debug)]
pub enum ChainSpec {
    Development,
    Testnet,
    Mainnet,
}

/// Get a chain config from a spec setting.
impl ChainSpec {
    pub(crate) fn load(self) -> Result<chain_spec::ChainSpec, String> {
        Ok(match self {
            ChainSpec::Development => chain_spec::development_config(),
            ChainSpec::Testnet => chain_spec::testnet_config(),
            ChainSpec::Mainnet => chain_spec::mainnet_config(),
        })
    }

    pub(crate) fn from(s: &str) -> Option<Self> {
        match s {
            "mainnet" | "" => Some(ChainSpec::Mainnet),
            "testnet" => Some(ChainSpec::Testnet),
            "dev" => Some(ChainSpec::Development),
            _ => None,
        }
    }
}

fn load_spec(id: &str) -> Result<Option<chain_spec::ChainSpec>, String> {
    Ok(match ChainSpec::from(id) {
        Some(spec) => Some(spec.load()?),
        None => None,
    })
}

#[derive(Debug)]
struct Directive {
    name: Option<String>,
    level: LevelFilter,
}

/// Parse a logging specification string (e.g: "crate1,crate2::mod3,crate3::x=error/foo") or (e.g: "info,target1=info,target2=debug")
/// and return a vector with log directives.
fn parse_spec(spec: &str) -> (Vec<Directive>, Option<LevelFilter>) {
    let mut dirs = Vec::new();

    let mut parts = spec.split('/');
    let mods = parts.next();
    let filter = parts.next().and_then(|s| FromStr::from_str(s).ok());
    if parts.next().is_some() {
        eprintln!(
            "warning: invalid logging spec '{}', ignoring it (too many '/'s)",
            spec
        );
        return (dirs, None);
    }
    mods.map(|m| {
        for s in m.split(',') {
            if s.len() == 0 {
                continue;
            }
            let mut parts = s.split('=');
            let (log_level, name) =
                match (parts.next(), parts.next().map(|s| s.trim()), parts.next()) {
                    (Some(part0), None, None) => {
                        // if the single argument is a log-level string or number,
                        // treat that as a global fallback
                        match part0.parse() {
                            Ok(num) => (num, None),
                            Err(_) => (LevelFilter::max(), Some(part0)),
                        }
                    }
                    (Some(part0), Some(""), None) => (LevelFilter::max(), Some(part0)),
                    (Some(part0), Some(part1), None) => match part1.parse() {
                        Ok(num) => (num, Some(part0)),
                        _ => {
                            eprintln!(
                                "warning: invalid logging spec '{}', \
                                 ignoring it",
                                part1
                            );
                            continue;
                        }
                    },
                    _ => {
                        eprintln!(
                            "warning: invalid logging spec '{}', \
                             ignoring it",
                            s
                        );
                        continue;
                    }
                };
            dirs.push(Directive {
                name: name.map(|s| s.to_string()),
                level: log_level,
            });
        }
    });

    let mut tmp_filter = LevelFilter::Off;
    for d in dirs.iter() {
        if d.name == None {
            if d.level > tmp_filter {
                tmp_filter = d.level;
            }
        }
    }

    let filter = if let Some(f) = filter {
        if f > tmp_filter {
            Some(f)
        } else {
            Some(tmp_filter)
        }
    } else {
        if tmp_filter == LevelFilter::Off {
            None
        } else {
            Some(tmp_filter)
        }
    };

    return (dirs, filter);
}

fn init_logger_log4rs(spec: &str, params: ChainXParams) -> Result<(), String> {
    use log4rs::{
        append::{
            console::ConsoleAppender,
            rolling_file::{
                policy::{
                    self,
                    compound::{roll, trigger},
                },
                RollingFileAppender,
            },
        },
        config,
        encode::pattern::PatternEncoder,
    };

    let (directives, filter) = parse_spec(spec);
    let filter = filter.unwrap_or(LevelFilter::Info);

    let (pattern1, pattern2) = if filter > LevelFilter::Info {
        (
            "{d(%Y-%m-%d %H:%M:%S:%3f)} {T} {h({l})} {t}  {m}\n",
            "{d(%Y-%m-%d %H:%M:%S:%3f)} {T} {l} {t}  {m}\n", // remove color
        )
    } else {
        (
            "{d(%Y-%m-%d %H:%M:%S:%3f)} {h({l})} {m}\n",
            "{d(%Y-%m-%d %H:%M:%S:%3f)} {l} {m}\n", // remove color
        )
    };

    let console = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(pattern1)))
        .build();

    let log = params.log_dir.clone() + "/" + &params.log_name;
    let log_file = if params.log_compression {
        log.clone() + ".gz"
    } else {
        log.clone()
    };

    if params.log_size == 0 {
        return Err("the `--log-size` can't be 0".to_string());
    }

    let trigger = trigger::size::SizeTrigger::new(1024 * 1024 * params.log_size);
    let roll_pattern = format!("{}.{{}}", log_file);
    let roll = roll::fixed_window::FixedWindowRoller::builder()
        .build(roll_pattern.as_str(), params.log_roll_count)
        .map_err(|e| format!("log rotate file:{:?}", e))?;

    let policy = policy::compound::CompoundPolicy::new(Box::new(trigger), Box::new(roll));
    let roll_file = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(pattern2)))
        .build(log, Box::new(policy))
        .map_err(|e| format!("{}", e))?;

    let mut tmp_builder = if params.log_console {
        config::Config::builder()
            .appender(config::Appender::builder().build("console", Box::new(console)))
            .appender(config::Appender::builder().build("roll", Box::new(roll_file)))
    } else {
        config::Config::builder()
            .appender(config::Appender::builder().build("roll", Box::new(roll_file)))
    };

    for d in directives {
        if let Some(name) = d.name {
            tmp_builder = tmp_builder.logger(config::Logger::builder().build(name, d.level));
        }
    }

    let root = if params.log_console {
        config::Root::builder()
            .appender("roll")
            .appender("console")
            .build(filter)
    } else {
        config::Root::builder().appender("roll").build(filter)
    };

    let log_config = tmp_builder
        .build(root)
        .expect("Construct log config failure");

    log4rs::init_config(log_config).expect("Initializing log config shouldn't be fail");
    Ok(())
}

pub fn run<I, T, E>(args: I, exit: E, version: cli::VersionInfo) -> error::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
    E: IntoExit,
{
    cli::parse_and_execute::<service::Factory, NoCustom, ChainXParams, _, _, _, _, _, _>(
        load_spec,
        &version,
        "ChainX",
        args,
        exit,
        |s, cli| {
            if cli.right.default_log {
                cli::init_logger(s);
                Ok(())
            } else {
                init_logger_log4rs(s, cli.right)
            }
        },
        |exit, _cli_args, custom_args, config| {
            info!("{}", version.name);
            info!("  version {}", config.full_version());
            info!("  by ChainX, 2018-2019");
            info!("Chain specification: {}", config.chain_spec.name());
            if let Some(id) = config.chain_spec.protocol_id() {
                info!("Chain protocol_id: {:}", id);
            } else {
                warn!("Not set protocol_id! may receive other blockchain network msg");
            }
            info!("Chain properties: {:?}", config.chain_spec.properties());
            info!("Node name: {}", config.name);
            info!("Roles: {:?}", config.roles);
            let runtime = RuntimeBuilder::new()
                .name_prefix("main-tokio-")
                .build()
                .map_err(|e| format!("{:?}", e))?;
            let executor = runtime.executor();

            substrate_rpc::set_cache_flag(custom_args.rpc_cache);

            if config.roles == ServiceRoles::AUTHORITY {
                let name = custom_args
                    .validator_name
                    .expect("if in AUTHORITY mode, must point the validator name!");
                info!("Validator name: {:}", name);
                set_validator_name(name);
            }

            match config.roles {
                ServiceRoles::LIGHT => run_until_exit(
                    runtime,
                    custom_args.ws_max_connections,
                    service::Factory::new_light(config, executor)
                        .map_err(|e| format!("{:?}", e))?,
                    exit,
                ),
                _ => run_until_exit(
                    runtime,
                    custom_args.ws_max_connections,
                    service::Factory::new_full(config, executor).map_err(|e| format!("{:?}", e))?,
                    exit,
                ),
            }
            .map_err(|e| format!("{:?}", e))
        },
    )
    .map_err(Into::into)
    .map(|_| ())
}

fn run_until_exit<T, C, E>(
    mut runtime: Runtime,
    ws_max_connections: usize,
    service: T,
    e: E,
) -> error::Result<()>
where
    T: Deref<Target = substrate_service::Service<C>> + native_rpc::Rpc,
    C: substrate_service::Components,
    E: IntoExit,
{
    let (exit_send, exit) = exit_future::signal();

    let executor = runtime.executor();
    let (_http, _ws) = service.start_rpc(ws_max_connections, executor.clone());
    cli::informant::start(&service, exit.clone(), executor.clone());

    let _ = runtime.block_on(e.into_exit());
    exit_send.fire();

    // we eagerly drop the service so that the internal exit future is fired,
    // but we need to keep holding a reference to the global telemetry guard
    let _telemetry = service.telemetry();
    drop(service);
    Ok(())
}

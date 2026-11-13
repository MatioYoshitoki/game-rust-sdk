//! Authoritative match SDK. Load a ConfigCatalog, retain its Runtime owner, and use Client.
//! One Runtime owns one match; the six Client methods exchange immutable protocol data.
//! See SDK.md for host responsibilities and current gameplay boundaries.
mod asset_contract;
mod combat_math;
mod config;
mod contact;
mod engine;
mod origin;
pub mod protocol;
pub mod replication;
mod result_contract;
mod runtime;
mod session;
mod status;
mod support;
mod two_members;
mod world;

pub use config::{Config, ConfigCatalog, config_hash};
pub use runtime::{Client, Runtime};
pub use support::{JSONL_LINE_LIMIT, sdk_info};

#[cfg(all(test, match_sdk_test_data))]
mod test_data;
#[cfg(all(test, match_sdk_test_data))]
mod verification;

//! Compatibility shims for call sites that will be removed with their owning
//! cloud-facing subsystems. They intentionally do not allocate identities,
//! collect data, spawn tasks, or communicate over the network.

use serde_json::Value;

pub fn init(_: bool) {}

pub fn get_distinct_id() -> &'static str {
    "local"
}

pub async fn capture_event(_: &str, _: Value) {}

pub fn capture_event_nonblocking(_: &'static str, _: Value) {}

pub fn check_macos_version() {}

pub fn track_api_usage(_: usize) {}

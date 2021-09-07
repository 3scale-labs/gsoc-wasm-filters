#![deny(clippy::all, clippy::cargo)]

const VM_ID: &str = "my_vm_id";

mod configuration;
mod filter;
mod log;
mod rand;
mod unique_callout;
#[cfg(not(feature = "unique_callout"))]
mod unique_callout_dummy;
mod utils;

// Full host event-batch release, conditional on the two pinned std contracts.
#![allow(unused_macros)]
#![feature(allocator_api)]
#[path = "completion_hash_reserve_contracts_v1.rs"]
mod reserve_contracts;
use std::collections::{HashMap, HashSet};
use vstd::std_specs::iter::IteratorSpec;
use vstd::prelude::*;

include!("completion_owner_schema_v1.rs");
include!("completion_event_batch_release_execution_v1.rs");

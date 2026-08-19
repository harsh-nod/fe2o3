#![forbid(unsafe_code)]

pub mod support;
mod valid_reference;

#[path = "invalid/accumulator_reset.rs"]
mod accumulator_reset;
#[path = "invalid/divergent_barrier.rs"]
mod divergent_barrier;
#[path = "invalid/duplicate_lane_c_write.rs"]
mod duplicate_lane_c_write;
#[path = "invalid/duplicate_lds_write.rs"]
mod duplicate_lds_write;
#[path = "invalid/expired_lds_epoch.rs"]
mod expired_lds_epoch;
#[path = "invalid/incorrect_alpha_beta_epilogue.rs"]
mod incorrect_alpha_beta_epilogue;
#[path = "invalid/incorrect_k_tail_zero_fill.rs"]
mod incorrect_k_tail_zero_fill;
#[path = "invalid/lds_read_before_initialization.rs"]
mod lds_read_before_initialization;
#[path = "invalid/missing_publish_barrier.rs"]
mod missing_publish_barrier;
#[path = "invalid/missing_reuse_barrier.rs"]
mod missing_reuse_barrier;
#[path = "invalid/overlapping_workgroup_c_tile.rs"]
mod overlapping_workgroup_c_tile;
#[path = "invalid/staged_read_before_wait.rs"]
mod staged_read_before_wait;
#[path = "invalid/unguarded_a_tail_load.rs"]
mod unguarded_a_tail_load;
#[path = "invalid/unguarded_b_tail_load.rs"]
mod unguarded_b_tail_load;
#[path = "invalid/unguarded_c_tail_store.rs"]
mod unguarded_c_tail_store;

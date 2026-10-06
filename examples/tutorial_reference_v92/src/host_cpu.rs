// Import only the shared host oracles, never the examples' typed device modules.
// Their own unit tests remain enabled in this standalone host test binary.
#![allow(dead_code)]

#[path = "host_attention.rs"]
pub mod attention;
#[path = "../../fill/src/reference.rs"]
pub mod fill;
#[path = "../../flash_attention_general_v1/src/reference.rs"]
pub mod flash_attention;
#[path = "../../gemm_autoresearch_v1/src/reference.rs"]
pub mod gemm;
#[path = "../../gfx950_gpt_oss_decode/src/reference.rs"]
pub mod gpt;
#[path = "host_low_precision.rs"]
pub mod low_precision;
#[path = "../../moe_grouped_expert_general_v1/src/reference.rs"]
pub mod moe_grouped;
#[path = "host_moe_top2.rs"]
pub mod moe_top2;
#[path = "../../row_softmax_general_v1/src/reference.rs"]
pub mod row_softmax;
#[path = "../../scalar_gemm_v1/src/harness.rs"]
pub mod scalar_gemm;
#[path = "../../tiled_gemm_general_v1/src/reference.rs"]
pub mod tiled_gemm;
#[path = "host_wave64.rs"]
pub mod wave64;
#[path = "host_workgroup.rs"]
pub mod workgroup;

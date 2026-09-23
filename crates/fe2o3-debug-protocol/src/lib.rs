#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod codec;
mod declared_target_codec_v1;
mod declared_target_v1;
mod diagnosis_v2;
mod hardware_stop_resources_v1;
mod hardware_v2;
mod kfd_checkpoint_qualification_v1;
mod live_gpu_v3;
mod model;
mod observed_query_codec_v1;
mod qualification_v1;
mod resource_queries_v1;
mod resource_queries_v2;
mod rocgdb_hardware_capture_v1;
mod rocgdb_mi_cli_v3;
mod rocgdb_mi_v3;
mod rocgdb_mi_v4;
mod rocgdb_mi_v5;
mod runtime_observations_v1;
mod source_variables_v2;

pub use codec::{
    ProtocolCodecErrorV1, decode_request_line_v1, decode_response_line_v1, encode_response_line_v1,
    read_request_line_v1,
};
pub use declared_target_codec_v1::*;
pub use declared_target_v1::*;
pub use diagnosis_v2::*;
pub use hardware_stop_resources_v1::*;
pub use hardware_v2::*;
pub use kfd_checkpoint_qualification_v1::*;
pub use live_gpu_v3::*;
pub use model::*;
pub use observed_query_codec_v1::*;
pub use qualification_v1::*;
pub use resource_queries_v1::*;
pub use resource_queries_v2::*;
pub use rocgdb_hardware_capture_v1::*;
pub use rocgdb_mi_cli_v3::*;
pub use rocgdb_mi_v3::*;
pub use rocgdb_mi_v4::*;
pub use rocgdb_mi_v5::*;
pub use runtime_observations_v1::*;
pub use source_variables_v2::*;

#[cfg(test)]
mod declared_target_tests;
#[cfg(test)]
mod observed_queries_tests;

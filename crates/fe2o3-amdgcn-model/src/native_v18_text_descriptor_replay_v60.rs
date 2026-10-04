//! Exact V18/V53 native text content, never semantic or artifact authority.
const PREFIX: &[u8] =
    b"\nmodule asm \".section .fe2o3.kd.v53,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";

include!("native_v18_text_descriptor_replay_family.rs");
pub use NativeTextDescriptorReplayError as NativeV18TextDescriptorReplayErrorV60;
pub use check_relation as check_native_v18_text_descriptor_relation_v60;

#[cfg(test)]
#[path = "native_v18_text_descriptor_replay_v60_tests.rs"]
mod tests;

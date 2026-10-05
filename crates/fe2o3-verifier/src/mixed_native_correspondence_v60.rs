//! V53 final native content correspondence, not protected proof authority.
use fe2o3_amdgcn_model::{
    NativeV18TextDescriptorReplayErrorV60 as NativeReplayError,
    check_native_v18_text_descriptor_relation_v60 as check_native_relation,
};
const DESCRIPTOR_BINDING: &str = "exact final graph, V53 descriptor and text kind";
include!("mixed_native_correspondence_family.rs");
pub use NativeCorrespondenceError as MixedNativeCorrespondenceErrorV60;
pub use check_correspondence as check_mixed_native_correspondence_v60;

#[cfg(test)]
#[path = "mixed_native_correspondence_v60_tests.rs"]
mod tests;

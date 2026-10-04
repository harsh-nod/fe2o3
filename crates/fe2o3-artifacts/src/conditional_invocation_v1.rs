//! The one shared inert schema lives below compiler-ffi in the dependency graph.
//! Re-exporting preserves type identity; this is not an artifact admission API.
pub use fe2o3_kernel_descriptor::conditional_invocation::*;

//! Fixed inherited peer admission, not protected-service or compiler qualification.
#![cfg(target_os = "linux")]

use fe2o3_compiler_execution_client::{
    CompilerExecutionClientErrorV2 as Error, CompilerExecutionClientV2 as Client,
};

include!("native_inherited/support.rs");

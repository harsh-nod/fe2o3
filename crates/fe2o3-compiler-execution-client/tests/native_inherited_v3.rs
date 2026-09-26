//! Fixed inherited peer admission, not protected-service or compiler qualification.
#![cfg(target_os = "linux")]

use fe2o3_compiler_execution_client::{
    CompilerExecutionClientErrorV3 as Error, CompilerExecutionClientV3 as Client,
};

include!("native_inherited/support.rs");

use super::*;
#[path = "process_native_error_adapter.rs"]
mod adapter;
adapter::native_error!(ProtectedIssuerLaunchErrorV2);

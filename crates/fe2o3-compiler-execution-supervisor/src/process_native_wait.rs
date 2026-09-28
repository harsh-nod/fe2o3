use super::*;
#[path = "process_native_wait_adapter.rs"]
mod adapter;
adapter::native_wait!(ProtectedIssuerBoundaryV2, ProtectedIssuerWaitV2);

//! Explicit server-local accounting over the existing address-free protocols.

#![forbid(unsafe_code)]

use super::*;
use crate::{RuntimeErrorV1, RuntimeWorkerRequestOwnerV1};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

/// Serves V1 with persistent server-local request accounting. The immediate
/// progress bound is unchanged; composed KFD backends must use V4 or V5.
///
/// ```compile_fail
/// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeWorkerRequestOwnerV1,
///     serve_runtime_request_owner_v1};
/// fn serve(owner: &mut RuntimeWorkerRequestOwnerV1<KfdRuntimeBackendV1>) {
///     let _ = serve_runtime_request_owner_v1(owner, std::io::empty(), std::io::sink());
/// }
/// ```
pub fn serve_runtime_request_owner_v1<B, R, W>(
    owner: &mut RuntimeWorkerRequestOwnerV1<B>,
    input: R,
    output: W,
) -> Result<(), RuntimeWorkerErrorV1>
where
    B: RuntimeWorkerV1ImmediateProgressBackendV1,
    R: Read,
    W: Write,
{
    serve(
        owner,
        input,
        output,
        RUNTIME_WORKER_HANDSHAKE_V1,
        dispatch_binary_request_v1_scoped,
    )
}

/// Serves V4 with persistent server-local request accounting. No request credit
/// or borrowed witness is transported, and client-side accounting is independent.
pub fn serve_runtime_request_owner_v4<B, R, W>(
    owner: &mut RuntimeWorkerRequestOwnerV1<B>,
    input: R,
    output: W,
) -> Result<(), RuntimeWorkerErrorV1>
where
    B: RuntimeFlushBackendV1 + RuntimeAsyncCopyBackendV1 + RuntimeCancellationBackendV1,
    R: Read,
    W: Write,
{
    serve(
        owner,
        input,
        output,
        RUNTIME_WORKER_HANDSHAKE_V4,
        dispatch_binary_request_v4_scoped,
    )
}

/// Serves V5 while retaining allocation/request custody in the borrowed owner.
/// A framing error or panic seals the owner without native cleanup or refund.
/// A clean empty shutdown frame ends framing only; release allocations explicitly
/// before recovering the backend for native teardown. This is not Worker V3
/// compiler/application authority or cross-process shared-root accounting.
///
/// ```no_run
/// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeWorkerRequestOwnerV1,
///     serve_runtime_request_owner_v5};
/// fn serve(backend: KfdRuntimeBackendV1) {
///     let mut owner = RuntimeWorkerRequestOwnerV1::open(backend).unwrap();
///     serve_runtime_request_owner_v5(&mut owner, std::io::stdin().lock(),
///         std::io::stdout().lock()).unwrap();
///     let mut backend = owner.try_into_backend().unwrap();
///     backend.shutdown_native_v1().unwrap();
/// }
/// ```
pub fn serve_runtime_request_owner_v5<B, R, W>(
    owner: &mut RuntimeWorkerRequestOwnerV1<B>,
    input: R,
    output: W,
) -> Result<(), RuntimeWorkerErrorV1>
where
    B: RuntimeFlushBackendV1
        + RuntimeAsyncCopyBackendV1
        + RuntimeCancellationBackendV1
        + RuntimeAtomicBackendV1
        + RuntimeCollectiveBackendV1,
    R: Read,
    W: Write,
{
    serve(
        owner,
        input,
        output,
        RUNTIME_WORKER_HANDSHAKE_V5,
        dispatch_binary_request_v5_scoped,
    )
}

fn serve<B: RuntimeBackendV1, R: Read, W: Write>(
    owner: &mut RuntimeWorkerRequestOwnerV1<B>,
    input: R,
    output: W,
    handshake: &[u8],
    dispatch: impl Fn(
        &mut B,
        &[u8],
        Option<&dyn Fn(u64) -> bool>,
    ) -> Result<Vec<u8>, RuntimeWorkerErrorV1>,
) -> Result<(), RuntimeWorkerErrorV1> {
    owner.require_live_v1()?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        serve_runtime_worker_with_handshake_v1(input, output, handshake, |request| {
            let response = dispatch_accounted(owner, request, &dispatch)?;
            if response.first() == Some(&RESPONSE_TERMINAL_V1) {
                owner.seal_v1();
            }
            Ok(response)
        })
    }));
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            owner.seal_v1();
            Err(error)
        }
        Err(payload) => {
            owner.seal_v1();
            resume_unwind(payload)
        }
    }
}

fn classify<T, E>(
    terminal: bool,
    result: Result<T, RuntimeErrorV1<E>>,
) -> Result<T, RuntimeBackendFailureV1<RuntimeErrorV1<E>>> {
    result.map_err(|error| {
        if terminal
            || matches!(
                error,
                RuntimeErrorV1::BackendTerminal(_) | RuntimeErrorV1::BackendProtocol(_)
            )
        {
            RuntimeBackendFailureV1::Terminal(error)
        } else if matches!(error, RuntimeErrorV1::BackendQuiescent(_)) {
            // The frozen wire cannot certify allocation-specific no-owner settlement.
            RuntimeBackendFailureV1::Quiescent(error)
        } else {
            RuntimeBackendFailureV1::Rejected(error)
        }
    })
}

fn dispatch_accounted<B: RuntimeBackendV1>(
    owner: &mut RuntimeWorkerRequestOwnerV1<B>,
    request: &[u8],
    dispatch: impl FnOnce(
        &mut B,
        &[u8],
        Option<&dyn Fn(u64) -> bool>,
    ) -> Result<Vec<u8>, RuntimeWorkerErrorV1>,
) -> Result<Vec<u8>, RuntimeWorkerErrorV1> {
    owner.require_live_v1()?;
    let mut input = BinaryCursorV1::new(request);
    match binary_u8_v1(&mut input)? {
        OP_ENUMERATE_DEVICES_V1 => {
            require_binary_end_v1(&input)?;
            encode_devices_response_v1(classify(owner.is_terminal_v1(), owner.enumerate_v1()))
        }
        OP_ALLOCATE_V1 => {
            let device = binary_u64_v1(&mut input)?;
            let kind = decode_memory_kind_v1(binary_u8_v1(&mut input)?)
                .map_err(|_| RuntimeWorkerErrorV1::Protocol("invalid memory kind"))?;
            let bytes = binary_u64_v1(&mut input)?;
            let alignment = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            let result = owner.allocate_v1(device, kind, bytes, alignment);
            encode_handle_response_v1(classify(owner.is_terminal_v1(), result))
        }
        OP_RELEASE_ALLOCATION_V1 => {
            let handle = binary_u64_v1(&mut input)?;
            require_binary_end_v1(&input)?;
            let result = owner.release_v1(handle);
            encode_unit_response_v1(classify(owner.is_terminal_v1(), result))
        }
        _ => owner.dispatch_backend_v1(request, |backend, request, owns| {
            dispatch(backend, request, Some(owns))
        }),
    }
}

#[cfg(test)]
mod tests;

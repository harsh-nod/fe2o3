//! Reuse the compiler's typed import policy and exact symbol-role constructor.
use super::*;
use crate::compiler_module_contract::{construct_symbol_manifest, validate_envelope_module_roles};
use crate::production_worker_handoff::derive_production_compiler_ffi_envelope;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerModuleKindV1,
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V3 as METADATA,
    MAX_COMPILER_FFI_ENVELOPE_BYTES_V1, MAX_COMPILER_MODULE_BYTES_V1,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V2, MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1,
    MAX_COMPILER_MODULE_SYMBOLS_V1,
};

pub(super) fn prepare(prefix: &mut Prefix, budget: &mut Budget<'_>) -> R<Module> {
    let output = prefix.chain.output();
    let text = &prefix.content.module;
    let graph_len = output.canonical().canonical_bytes().len();
    let extent = MAX_COMPILER_MODULE_HANDOFF_BYTES_V2
        .checked_sub(MAX_COMPILER_MODULE_BYTES_V1)
        .and_then(|n| n.checked_add(text.llvm_ir().len()))
        .ok_or(Resource::Arithmetic)?;
    let (storage, work) = quote(graph_len, output.module().functions.len(), extent)?;
    budget.reserve_storage(storage)?;
    budget.charge_work(work)?;
    let original = *prefix
        .preparation
        .ranked
        .materialized()
        .executable()
        .canonical()
        .identity()
        .digest();
    let bindings = &mut prefix.preparation.bindings;
    let target = bindings.rustc_target.device_target();
    let envelope = derive_production_compiler_ffi_envelope(
        target,
        output.module(),
        text,
        bindings.transaction.compiler_ffi_envelope.take(),
        original,
    )
    .map_err(Error::Worker)?;
    validate_envelope_module_roles(&envelope, text).map_err(Error::Roles)?;
    let manifest = construct_symbol_manifest(text).map_err(Error::Manifest)?;
    let module = Module::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        manifest,
        text.llvm_ir().as_bytes(),
    )
    .map_err(Error::Module)?;
    budget.reserve_storage(module.backing_capacity().saturating_sub(extent))?;
    Ok(module)
}

// Versioned logical allowance for unchanged bounded constructors. The graph
// factor includes the existing per-reachable-function linear lookup, rather
// than claiming reachability is linear. The manifest comparison schedule uses
// the same pinned BTreeMap bound as native handoff decoding. All old routes keep
// their own debit schedules. Reaudit when these implementations/limits change.
fn quote(graph: usize, functions: usize, backing: usize) -> R<(usize, usize)> {
    let storage = graph
        .checked_mul(4)
        .and_then(|n| n.checked_add(METADATA))
        .and_then(|n| n.checked_add(backing))
        .ok_or(Resource::Arithmetic)?;
    let graph_work = functions
        .checked_add(1)
        .and_then(|n| n.checked_mul(graph))
        .and_then(|n| n.checked_mul(320))
        .ok_or(Resource::Arithmetic)?;
    let work = graph_work
        .checked_add(128 * MAX_COMPILER_FFI_ENVELOPE_BYTES_V1)
        .and_then(|n| n.checked_add(320 * MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1))
        .and_then(|n| n.checked_add(4096 * MAX_COMPILER_MODULE_SYMBOLS_V1))
        .and_then(|n| n.checked_add(4 * 1024 * 1024))
        .and_then(|n| backing.checked_mul(8).and_then(|b| n.checked_add(b)))
        .ok_or(Resource::Arithmetic)?;
    Ok((storage, work))
}

//! Reuse the compiler's typed import policy and exact symbol-role constructor.
use super::*;
use crate::compiler_module_contract::{construct_symbol_manifest, validate_envelope_module_roles};
use crate::production_worker_handoff::derive_production_compiler_ffi_envelope;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerModuleKindV1,
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V3 as METADATA,
    MAX_COMPILER_FFI_ENVELOPE_BYTES_V1, MAX_COMPILER_MODULE_BYTES_V1,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V2, MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1,
    MAX_COMPILER_MODULE_SYMBOLS_V1, MAX_DEVICE_FFI_TARGET_BYTES_V1,
};

const MODULE_FIXED: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES_V2
    - MAX_COMPILER_MODULE_BYTES_V1
    - MAX_COMPILER_FFI_ENVELOPE_BYTES_V1
    - MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1
    - MAX_DEVICE_FFI_TARGET_BYTES_V1;
const MANIFEST_FIXED: usize = 45;
const _: () = {
    assert!(MODULE_FIXED == 123);
    assert!(MAX_COMPILER_MODULE_SYMBOLS_V1 == 16384);
    assert!(MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 == 524288);
    assert!(size_of::<String>() <= 24);
};

pub(super) fn prepare(prefix: &mut Prefix, budget: &mut Budget<'_>) -> R<Module> {
    let output = prefix.chain.output();
    let text = &prefix.content.module;
    let graph_len = output.canonical().canonical_bytes().len();
    let (rows, manifest_extent) = manifest_extent(text, budget)?;
    let extent = module_extent(
        MAX_DEVICE_FFI_TARGET_BYTES_V1,
        MAX_COMPILER_FFI_ENVELOPE_BYTES_V1,
        manifest_extent,
        text.llvm_ir().len(),
    )?;
    let (storage, work) = quote(
        graph_len,
        output.module().functions.len(),
        rows,
        manifest_extent,
        extent,
    )?;
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
    if manifest.canonical_bytes().len() != manifest_extent {
        return Err(Error::Mismatch("conditional native manifest extent policy"));
    }
    let extent = module_extent(
        bindings.rustc_target.profile().device_target().len(),
        envelope.canonical_bytes().len(),
        manifest_extent,
        text.llvm_ir().len(),
    )?;
    budget.reserve_storage(extent)?;
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
fn quote(
    graph: usize,
    functions: usize,
    rows: usize,
    manifest: usize,
    backing: usize,
) -> R<(usize, usize)> {
    let storage = graph
        .checked_mul(4)
        .and_then(|n| n.checked_add(METADATA))
        .ok_or(Resource::Arithmetic)?;
    let graph_work = functions
        .checked_add(1)
        .and_then(|n| n.checked_mul(graph))
        .and_then(|n| n.checked_mul(320))
        .ok_or(Resource::Arithmetic)?;
    let work = graph_work
        .checked_add(128 * MAX_COMPILER_FFI_ENVELOPE_BYTES_V1)
        .and_then(|n| manifest.checked_mul(320).and_then(|m| n.checked_add(m)))
        .and_then(|n| rows.checked_mul(4096).and_then(|r| n.checked_add(r)))
        .and_then(|n| n.checked_add(128 * 127 * (128 + 3 * 32 + 4)))
        .and_then(|n| n.checked_add(4 * 1024 * 1024))
        .and_then(|n| backing.checked_mul(8).and_then(|b| n.checked_add(b)))
        .ok_or(Resource::Arithmetic)?;
    Ok((storage, work))
}

fn module_extent(target: usize, envelope: usize, manifest: usize, llvm: usize) -> R<usize> {
    [target, envelope, manifest, llvm]
        .into_iter()
        .try_fold(MODULE_FIXED, |n, field| {
            n.checked_add(field).ok_or(Resource::Arithmetic.into())
        })
}

fn manifest_extent(
    text: &crate::kernel_ir_codegen::InertCompilerModuleTextV1,
    budget: &mut Budget<'_>,
) -> R<(usize, usize)> {
    let groups = [
        (text.kernel_entries(), 0usize),
        (text.kernel_entries(), 3),
        (text.device_ffi_exports(), 0),
        (text.internal_helpers(), 0),
        (text.external_declarations(), 0),
    ];
    let rows = groups
        .iter()
        .try_fold(0usize, |n, (names, _)| n.checked_add(names.len()))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(
        rows.checked_mul(4)
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if rows > MAX_COMPILER_MODULE_SYMBOLS_V1 {
        return Err(Error::Mismatch("conditional native manifest row bound"));
    }
    let mut extent = MANIFEST_FIXED;
    for (names, suffix) in groups {
        for name in names {
            extent = extent
                .checked_add(5 + suffix)
                .and_then(|n| n.checked_add(name.len()))
                .ok_or(Resource::Arithmetic)?;
        }
    }
    if extent > MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1 {
        return Err(Error::Mismatch("conditional native manifest byte bound"));
    }
    Ok((rows, extent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditional_native_module_quote_pins_checked_costs_and_overflow() {
        let (storage, work) = quote(100, 2, 3, 75, 200).unwrap();
        assert_eq!(storage, METADATA + 400);
        assert_eq!(
            work,
            320 * (100 * 3 + 75)
                + 4096 * 3
                + 8 * 200
                + 128 * MAX_COMPILER_FFI_ENVELOPE_BYTES_V1
                + 128 * 127 * (128 + 3 * 32 + 4)
                + 4 * 1024 * 1024
        );
        for args in [
            (usize::MAX, 1, 1, 1, 1),
            (1, usize::MAX, 1, 1, 1),
            (1, 1, usize::MAX, 1, 1),
            (1, 1, 1, usize::MAX, 1),
            (1, 1, 1, 1, usize::MAX),
        ] {
            assert!(matches!(
                quote(args.0, args.1, args.2, args.3, args.4),
                Err(Error::Resource(Resource::Arithmetic))
            ));
        }
        assert!(matches!(
            module_extent(1, usize::MAX, 1, 1),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    }
}

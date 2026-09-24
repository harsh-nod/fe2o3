//! Complete original/final/root joins; no backend access or reconstructed proof.
use super::*;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerModuleKindV1, CompilerModuleSymbolRoleV1 as Symbol,
};
use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3 as MAX_ROOTS, MultiRootProofRosterKindV3 as Kind,
};
use fe2o3_kernel_descriptor::{
    AccessMode as Access, AliasSemantics as Alias, BlockSizeV1, DeviceDescriptorTableV1 as Table,
    DeviceLayoutDescriptorV1 as Layout, KernelDescriptorV1 as Kernel,
    OwnershipSemantics as Ownership, PhysicalAbiComponentKind as Component, ScalarTypeV1 as Scalar,
    SourceTypeDescriptorV1 as SourceType,
};
use fe2o3_kernel_ir::{
    AccessMode as KirAccess, AddressSpace, FormalMemoryObligations, FunctionRole,
    InertFormalMemoryReceiptFormatV4 as Formal, MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
    ScalarType as KirScalar, Type,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiPassModeV1 as Mode, SemanticFunctionDeclV1 as Function,
    SemanticSourceArgumentOwnershipV1 as SourceOwnership,
};

pub(super) fn subject(graph: &Graph, catalog: &Catalog) -> R<Subject> {
    Subject::new(
        *graph.canonical().identity().digest(),
        graph.canonical().identity().canonical_length(),
        *catalog.digest(),
        u64::try_from(catalog.canonical_bytes().len()).map_err(|_| Resource::Arithmetic)?,
    )
    .map_err(E::NativeSubject)
}

pub(super) fn profile(native: &Native) -> R<Profile> {
    use fe2o3_compiler_ffi::DeviceTargetV1;
    if native.kind() != CompilerModuleKindV1::LlvmTextIr
        || native.code_object_version() != CodeObjectVersion::V6
        || native.module_bytes().is_empty()
    {
        return Err(E::Mismatch("LLVM text/COV6 native output"));
    }
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        if DeviceTargetV1::parse(profile.device_target()).ok() == Some(native.target()) {
            return Ok(profile);
        }
    }
    Err(E::Mismatch("exact production target"))
}

fn copy(bytes: &[u8], budget: &mut Budget<'_>) -> R<Vec<u8>> {
    budget.reserve_storage(
        size_of::<Vec<u8>>()
            .checked_add(bytes.len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.charge_work(bytes.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
    let mut value = Vec::new();
    value
        .try_reserve_exact(bytes.len())
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        value
            .capacity()
            .checked_sub(bytes.len())
            .ok_or(Resource::Accounting)?,
    )?;
    value.extend_from_slice(bytes);
    Ok(value)
}

pub(super) fn association(frame: &Frame<'_>, budget: &mut Budget<'_>) -> R<()> {
    scoped(budget, |budget| {
        let wire = frame.field(Field::OriginalInputV4);
        codec::<Association>(wire.len(), budget)?;
        let association = Association::decode(wire).map_err(E::Association)?;
        bytes(
            association.verus_execution_evidence(),
            frame.field(Field::OriginalVerus),
            "V4 exact signed Verus bytes",
            budget,
        )?;
        let declared = association.inputs();
        macro_rules! identity {
            ($receipt:ty, $field:ident, $wanted:expr) => {{
                scoped(budget, |budget| {
                    let raw = frame.field(Field::$field);
                    codec::<$receipt>(raw.len(), budget)?;
                    budget.reserve_storage(HASH_STORAGE)?;
                    // All five pinned V3 domains are shorter than 64 bytes;
                    // cover framing, SHA padding/state and the zero-digest test.
                    budget.charge_work(256)?;
                    let actual = <$receipt>::from_canonical_preimage(copy(raw, budget)?)
                        .map_err(E::Lineage)?;
                    budget.charge_work(80)?;
                    let expected = $wanted;
                    if *actual.identity().sha256() != expected.sha256()
                        || actual.identity().byte_len() != expected.byte_len()
                    {
                        return Err(E::Mismatch(concat!(
                            "V4 ",
                            stringify!($field),
                            " identity/length"
                        )));
                    }
                    drop(actual);
                    Ok(())
                })?;
            }};
        }
        identity!(
            InertCanonicalSemanticMirReceiptV3,
            SemanticMir,
            declared.semantic_mir()
        );
        identity!(
            InertMiddleEndReceiptV3,
            OriginalMiddleEnd,
            declared.middle_end()
        );
        identity!(InertKernelIrReceiptV3, OriginalNative, declared.kernel_ir());
        identity!(
            InertMirToKirCorrespondenceReceiptV3,
            OriginalCorrespondence,
            declared.mir_to_kir_correspondence()
        );
        identity!(
            InertFormalMemoryReceiptV3,
            OriginalFormalMemory,
            declared.formal_memory()
        );
        drop(association);
        Ok(())
    })
}

pub(super) fn formal(
    inputs: &Inputs<'_>,
    graph: &Graph,
    subject: &Subject,
    roster: &Roster,
    reports: &[FormalMemoryObligations],
    budget: &mut Budget<'_>,
) -> R<()> {
    let count = inputs.semantic.roots().len();
    budget.charge_work(
        roster
            .canonical_bytes()
            .len()
            .checked_mul(count.checked_add(1).ok_or(Resource::Arithmetic)?)
            .and_then(|n| n.checked_add(graph.canonical().canonical_bytes().len()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if !(1..=MAX_ROOTS).contains(&count)
        || roster.root_count() != count
        || roster.kind() != Kind::FormalMemory
        || reports.len() != count
        || graph.module().kernels.len() != count
        || inputs.launch.roots().len() != count
        || roster.semantic_mir_sha256() != *inputs.semantic.semantic_sha256().as_bytes()
        || roster.native_neutral_subject() != subject
        || roster.roster_identity() != inputs.middle.roster_identity()
        || roster.canonical_kernel_order() != inputs.middle.canonical_kernel_order()
    {
        return Err(E::Mismatch("complete fresh formal roster"));
    }
    budget.reserve_storage(size_of::<[bool; MAX_ROOTS]>())?;
    let mut seen = [false; MAX_ROOTS];
    for ordinal in 0..count {
        let row = roster.root(ordinal).ok_or(E::Mismatch("formal row"))?;
        let signed = inputs
            .middle
            .root(ordinal)
            .ok_or(E::Mismatch("signed row"))?;
        let launch = inputs.launch.roots()[ordinal];
        budget.charge_work(
            graph
                .canonical()
                .canonical_bytes()
                .len()
                .checked_add(192)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut matches = graph
            .module()
            .kernels
            .iter()
            .enumerate()
            .filter(|(_, kernel)| kernel.id.as_str() == signed.export_symbol());
        let (kernel_index, kernel) = matches.next().ok_or(E::Mismatch("formal kernel"))?;
        if matches.next().is_some()
            || seen[kernel_index]
            || row.semantic_root() != inputs.semantic.roots()[ordinal].index()
            || row.semantic_root_identity() != *launch.semantic_root_identity().as_bytes()
            || row.kernel_binding() != launch.kernel_binding()
            || row.source_rank() != launch.source_rank()
            || Some(row.workgroup()) != launch.source_launch().exact_workgroup()
            || row.logical_name() != signed.logical_name()
            || row.export_symbol() != signed.export_symbol()
            || row.kernel_id() != kernel.id.as_str()
            || reports[kernel_index].kernel() != &kernel.id
            || reports[kernel_index].entry() != &kernel.entry
        {
            return Err(E::Mismatch("fresh formal root axes"));
        }
        seen[kernel_index] = true;
        scoped(budget, |budget| {
            budget.reserve_storage(
                MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1
                    .checked_add(size_of::<Formal>())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            budget.charge_work(MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1)?;
            let expected = Formal::from_current_obligations(&reports[kernel_index])
                .map_err(E::FormalReceipt)?;
            bytes(
                row.payload(),
                expected.canonical_bytes(),
                "fresh formal payload",
                budget,
            )?;
            drop(expected);
            Ok(())
        })?;
    }
    Ok(())
}

fn scalar(value: Scalar) -> KirScalar {
    match value {
        Scalar::I8 => KirScalar::I8,
        Scalar::U8 => KirScalar::U8,
        Scalar::I16 => KirScalar::I16,
        Scalar::U16 => KirScalar::U16,
        Scalar::I32 => KirScalar::I32,
        Scalar::U32 => KirScalar::U32,
        Scalar::I64 => KirScalar::I64,
        Scalar::U64 => KirScalar::U64,
        Scalar::F16 => KirScalar::F16,
        Scalar::F32 => KirScalar::F32,
        Scalar::F64 => KirScalar::F64,
    }
}

pub(super) fn argument_kind(source: &SourceType, ty: &Type, access: Access) -> bool {
    let element = scalar(source.scalar_type());
    match ty {
        Type::Scalar(actual) => {
            source.is_scalar() && *actual == element && access == Access::ByValue
        }
        Type::Slice(actual) => {
            actual.address_space == AddressSpace::Global
                && actual.element.as_scalar() == Some(element)
                && match (
                    source.is_shared_slice(),
                    source.is_disjoint_slice(),
                    access,
                    actual.access,
                ) {
                    (true, false, Access::ReadOnly, KirAccess::ReadOnly) => true,
                    (false, true, Access::WriteOnly, KirAccess::WriteOnly)
                    | (false, true, Access::ReadWrite, KirAccess::ReadWrite) => true,
                    _ => false,
                }
        }
        Type::Pointer(actual) => {
            source.is_global_mut_pointer()
                && actual.address_space == AddressSpace::Global
                && actual.pointee.as_scalar() == Some(element)
                && actual.access == KirAccess::ReadWrite
                && access == Access::ReadWrite
        }
        _ => false,
    }
}

fn arguments(
    table: &Table,
    descriptor: &Kernel,
    semantic: &Semantic,
    function: &Function,
    original: &fe2o3_kernel_ir::Function,
    output: &fe2o3_kernel_ir::Function,
    budget: &mut Budget<'_>,
) -> R<()> {
    let args = descriptor.arguments();
    let abi = function.abi();
    if original.signature != output.signature
        || !output.signature.results.is_empty()
        || args.len() != output.signature.parameters.len()
        || args.len() != abi.source_input_types().len()
        || args.len() != abi.adjusted_arguments().len()
        || args.len() != abi.source_argument_ownership().len()
    {
        return Err(E::Mismatch("complete semantic/N/F/descriptor signature"));
    }
    let mut offset = 0u32;
    let mut kernarg_alignment = 1u32;
    for (index, argument) in args.iter().enumerate() {
        budget.charge_work(
            table
                .type_records()
                .len()
                .checked_add(table.layout_records().len())
                .and_then(|n| n.checked_mul(80))
                .and_then(|n| n.checked_add(128))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let source = table
            .type_records()
            .iter()
            .find(|r| r.identity() == argument.source_type())
            .ok_or(E::Mismatch("descriptor source type"))?
            .descriptor();
        let layout = table
            .layout_records()
            .iter()
            .find(|r| r.identity() == argument.device_layout())
            .ok_or(E::Mismatch("descriptor device layout"))?
            .descriptor();
        let expected_layout = if source.is_scalar() {
            Layout::scalar(source.scalar_type())
        } else if source.is_shared_slice() {
            Layout::shared_slice(source.scalar_type())
        } else if source.is_disjoint_slice() {
            Layout::disjoint_slice(source.scalar_type())
        } else {
            Layout::global_mut_pointer(source.scalar_type())
        };
        let semantic_type = semantic
            .types()
            .get(abi.source_input_types()[index].index() as usize)
            .ok_or(E::Mismatch("semantic argument type"))?;
        let adjusted = &abi.adjusted_arguments()[index];
        let slice = source.is_shared_slice() || source.is_disjoint_slice();
        let (ownership, alias, source_ownership) = if source.is_scalar() {
            (Ownership::ByValue, Alias::Value, SourceOwnership::ByValue)
        } else if source.is_shared_slice() {
            (
                Ownership::SharedBorrow,
                Alias::SharedReadOnly,
                SourceOwnership::SharedBorrow,
            )
        } else {
            (
                Ownership::UniqueBorrow,
                Alias::Exclusive,
                SourceOwnership::ExclusiveOwner,
            )
        };
        if argument.source_index() as usize != index
            || layout != &expected_layout
            || argument.ownership() != ownership
            || argument.alias() != alias
            || abi.source_argument_ownership()[index] != source_ownership
            || !argument_kind(
                source,
                &output.signature.parameters[index],
                argument.access(),
            )
            || semantic_type.layout().size_bytes() != Some(u64::from(layout.size_bytes()))
            || semantic_type.layout().alignment_bytes() != u64::from(layout.alignment_bytes())
            || adjusted.ty() != abi.source_input_types()[index]
            || !matches!(
                (slice, adjusted.mode()),
                (true, Mode::Pair { .. }) | (false, Mode::Direct(_))
            )
        {
            return Err(E::Mismatch(
                "exact source ownership/layout/physical argument",
            ));
        }
        let alignment = u32::from(layout.alignment_bytes());
        kernarg_alignment = kernarg_alignment.max(alignment);
        offset = offset
            .checked_add(alignment - 1)
            .ok_or(Resource::Arithmetic)?
            / alignment
            * alignment;
        let mut components = argument.physical_components();
        let first = if source.is_scalar() {
            (
                Component::ScalarByValue(source.scalar_type()),
                offset,
                layout.size_bytes(),
                layout.alignment_bytes(),
            )
        } else {
            (Component::GlobalPointer, offset, 8, 8)
        };
        if components.next() != Some(first)
            || (slice
                && components.next()
                    != Some((
                        Component::SliceLengthU64,
                        offset.checked_add(8).ok_or(Resource::Arithmetic)?,
                        8,
                        8,
                    )))
            || components.next().is_some()
        {
            return Err(E::Mismatch("exact physical argument components"));
        }
        offset = offset
            .checked_add(u32::from(layout.size_bytes()))
            .ok_or(Resource::Arithmetic)?;
    }
    offset = offset
        .checked_add(kernarg_alignment - 1)
        .ok_or(Resource::Arithmetic)?
        / kernarg_alignment
        * kernarg_alignment;
    let layout = descriptor.abi_layout();
    if layout.explicit_argument_size() != offset
        || layout.kernarg_segment_size() != offset.checked_add(256).ok_or(Resource::Arithmetic)?
        || layout.kernarg_segment_alignment() != kernarg_alignment
    {
        return Err(E::Mismatch("exact complete kernarg layout"));
    }
    Ok(())
}

pub(super) fn capabilities(
    output: &Graph,
    descriptor: &Descriptor,
    budget: &mut Budget<'_>,
) -> R<()> {
    check_canonical_v12_descriptor_capabilities_v1(output, descriptor.table(), budget)
        .map_err(E::Capabilities)
}

pub(super) fn roots(
    frame: &Frame<'_>,
    inputs: &Inputs<'_>,
    output: &Graph,
    native: &Native,
    descriptor: &Descriptor,
    budget: &mut Budget<'_>,
) -> R<()> {
    let count = inputs.semantic.roots().len();
    let table = descriptor.table();
    if !(1..=MAX_ROOTS).contains(&count)
        || frame.root_count() as usize != count
        || inputs.original.module().kernels.len() != count
        || output.module().kernels.len() != count
        || table.kernels().len() != count
        || inputs.launch.roots().len() != count
        || table.device_target() != native.target()
        || table.code_object_version() != CodeObjectVersion::V6
        || table.canonical_code_object_digest().as_bytes() != &[0; 32]
    {
        return Err(E::Mismatch("complete four-axis root/target roster"));
    }
    let scan = inputs
        .original
        .canonical()
        .canonical_bytes()
        .len()
        .checked_add(output.canonical().canonical_bytes().len())
        .and_then(|n| n.checked_add(inputs.semantic.canonical_encoding().len()))
        .and_then(|n| n.checked_add(descriptor.canonical_bytes().len()))
        .and_then(|n| n.checked_add(native.symbol_manifest().canonical_bytes().len()))
        .ok_or(Resource::Arithmetic)?;
    // The root reader's working extent and returned row coexist with our seen
    // tables. The enclosing scope refunds only after every borrowed row drops.
    let root_working = READ_STORAGE
        .checked_add(size_of::<
            fe2o3_compiler_ffi::InertRefinedForwardingRootRefV1<'_>,
        >())
        .and_then(|n| n.checked_add(size_of::<[[bool; MAX_ROOTS]; 3]>()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(root_working)?;
    let mut seen = [[false; MAX_ROOTS]; 3];
    for ordinal in 0..count {
        // Includes all nested shape/name equality and the bounded per-row scans.
        budget.charge_work(scan.checked_add(512).ok_or(Resource::Arithmetic)?)?;
        let limit = budget.storage_limit();
        let row = frame
            .root(ordinal as u32, limit, |w| budget.charge_work(w))
            .map_err(E::Framing)?;
        let n = row.original_kernel_ordinal as usize;
        let f = row.final_kernel_ordinal as usize;
        let d = row.descriptor_ordinal as usize;
        if [n, f, d].into_iter().any(|i| i >= count) || seen[0][n] || seen[1][f] || seen[2][d] {
            return Err(E::Mismatch("four-axis root permutation"));
        }
        seen[0][n] = true;
        seen[1][f] = true;
        seen[2][d] = true;
        let semantic_id = inputs.semantic.roots()[ordinal];
        let semantic = inputs
            .semantic
            .functions()
            .get(semantic_id.index() as usize)
            .ok_or(E::Mismatch("semantic root ordinal"))?;
        let source = semantic
            .kernel_entry()
            .ok_or(E::Mismatch("semantic kernel entry"))?;
        let launch = inputs.launch.roots()[ordinal];
        let layout = launch.layout();
        let signed = inputs
            .middle
            .root(ordinal)
            .ok_or(E::Mismatch("signed root"))?;
        let original = &inputs.original.module().kernels[n];
        let final_kernel = &output.module().kernels[f];
        let descriptor = &table.kernels()[d];
        let original_function = inputs
            .original
            .module()
            .functions
            .get(row.original_function as usize)
            .ok_or(E::Mismatch("original function ordinal"))?;
        let final_function = output
            .module()
            .functions
            .get(row.final_function as usize)
            .ok_or(E::Mismatch("final function ordinal"))?;
        if row.semantic_root != semantic_id.index()
            || row.semantic_function_identity != *semantic.identity().as_bytes()
            || row.semantic_function_identity != *launch.semantic_root_identity().as_bytes()
            || row.source_kernel_binding != *source.kernel_binding_identity().as_bytes()
            || row.source_kernel_binding != launch.kernel_binding()
            || row.descriptor_kernel_id != row.source_kernel_binding
            || descriptor.kernel_id().as_bytes() != &row.descriptor_kernel_id
            || row.source_rank != launch.source_rank()
            || row.exact_workgroup != launch.source_launch().exact_workgroup()
            || row.exact_workgroup != Some(signed.workgroup())
            || row.source_max_grid != launch.source_launch().max_grid()
            || row.grid_identity != layout.grid_identity()
            || row.global_extents != layout.global_extents()
            || row.workgroup_extents != layout.workgroup_extents()
            || row.subgroup_size != layout.subgroup_size()
            || row.full_physical_workgroups != layout.full_physical_workgroups()
            || row.export_name.as_bytes() != source.export_symbol().as_bytes()
            || row.export_name != signed.export_symbol()
            || row.logical_name != signed.logical_name()
            || row.export_name != original.id.as_str()
            || row.export_name != final_kernel.id.as_str()
            || original.entry != original_function.id
            || final_kernel.entry != final_function.id
            || original_function.role != FunctionRole::KernelEntry
            || final_function.role != FunctionRole::KernelEntry
            || row.export_name != descriptor.entry_name().as_str()
            || row.logical_name != descriptor.logical_name().as_str()
            || descriptor.descriptor_symbol().as_str().strip_suffix(".kd") != Some(row.export_name)
            || inputs.middle.canonical_kernel_order().get(d).copied() != Some(ordinal as u32)
        {
            return Err(E::Mismatch("exact semantic/N/F/descriptor root axes"));
        }
        let wg = row
            .exact_workgroup
            .ok_or(E::Mismatch("exact source workgroup"))?;
        let descriptor_launch = descriptor.launch();
        let BlockSizeV1::Exact(block) = descriptor_launch.block_size() else {
            return Err(E::Mismatch("exact descriptor workgroup"));
        };
        let grid = descriptor_launch.max_grid();
        let static_resources = source
            .source_contract()
            .resources()
            .map(|r| {
                (
                    r.static_shared_memory_bytes(),
                    r.max_dynamic_shared_memory_bytes(),
                )
            })
            .unwrap_or_default();
        if descriptor_launch.rank() != row.source_rank
            || [block.x(), block.y(), block.z()] != wg
            || [grid.x(), grid.y(), grid.z()] != row.source_max_grid
            || descriptor_launch.max_flat_workgroup_size()
                != wg
                    .into_iter()
                    .try_fold(1u32, u32::checked_mul)
                    .ok_or(Resource::Arithmetic)?
            || (
                descriptor_launch.static_shared_memory_bytes(),
                descriptor_launch.max_dynamic_shared_memory_bytes(),
            ) != static_resources
            || final_kernel.domain.rank() != row.source_rank
            || final_kernel.workgroup_size.map(|w| [w.x, w.y, w.z]) != Some(wg)
        {
            return Err(E::Mismatch("exact source/descriptor/F launch"));
        }
        arguments(
            table,
            descriptor,
            inputs.semantic,
            semantic,
            original_function,
            final_function,
            budget,
        )?;
    }
    symbols(output, native, table, budget)
}

fn symbols(output: &Graph, native: &Native, table: &Table, budget: &mut Budget<'_>) -> R<()> {
    let manifest = native.symbol_manifest();
    budget.reserve_storage(size_of::<[usize; 5]>())?;
    let mut counts = [0usize; 5];
    for function in &output.module().functions {
        budget.charge_work(
            manifest
                .canonical_bytes()
                .len()
                .checked_add(function.id.as_str().len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let (role, index) = match function.role {
            FunctionRole::KernelEntry => (Symbol::KernelEntry, 0),
            FunctionRole::InternalHelper => (Symbol::InternalHelper, 1),
            FunctionRole::DeviceFfiExport => (Symbol::DeviceFfiExport, 2),
            FunctionRole::ExternalImport => (Symbol::UnresolvedExternalImport, 3),
        };
        if function.role != FunctionRole::KernelEntry
            && !manifest.symbols(role).any(|s| s == function.id.as_str())
        {
            return Err(E::Mismatch("complete F symbol manifest"));
        }
        counts[index] += 1;
    }
    for descriptor in table.kernels() {
        budget.charge_work(
            manifest
                .canonical_bytes()
                .len()
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?,
        )?;
        if !manifest
            .symbols(Symbol::KernelDescriptor)
            .any(|s| s == descriptor.descriptor_symbol().as_str())
            || !manifest
                .symbols(Symbol::KernelEntry)
                .any(|s| s == descriptor.entry_name().as_str())
        {
            return Err(E::Mismatch("complete descriptor symbol manifest"));
        }
        counts[4] += 1;
    }
    for (role, count) in [
        Symbol::KernelEntry,
        Symbol::InternalHelper,
        Symbol::DeviceFfiExport,
        Symbol::UnresolvedExternalImport,
        Symbol::KernelDescriptor,
    ]
    .into_iter()
    .zip(counts)
    {
        budget.charge_work(1)?;
        if manifest.role_count(role) != count {
            return Err(E::Mismatch("no extra native symbols"));
        }
    }
    Ok(())
}

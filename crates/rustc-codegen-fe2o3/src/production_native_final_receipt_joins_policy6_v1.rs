//! Private receipt-to-owner component. The consuming caller first replays its
//! complete signed native owner. Component success alone grants no such custody.
use super::super::root_index::{ExactRootNameIndexV1, RootNameMatchV1};
use super::*;
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1 as Subject, MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3 as MAX_ROOTS,
    MultiRootProofRosterKindV3 as Kind, MultiRootProofRosterTranscriptV3 as Roster,
    NativeNeutralModuleRefV1,
};
use fe2o3_kernel_ir::{
    FormalMemoryObligations, InertFormalMemoryReceiptFormatV4 as Formal,
    MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
};

pub(super) fn subject(output: &Graph, catalog: &Catalog) -> R<Subject> {
    Subject::new(
        *output.canonical().identity().digest(),
        output.canonical().identity().canonical_length(),
        *catalog.digest(),
        u64::try_from(catalog.canonical_bytes().len()).map_err(|_| Resource::Arithmetic)?,
    )
    .map_err(|_| E::Mismatch("final native subject"))
}

fn same_bytes(left: &[u8], right: &[u8], budget: &mut Budget<'_>, field: &'static str) -> R<()> {
    budget.charge_work(
        left.len()
            .checked_add(right.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if left != right {
        return Err(E::Mismatch(field));
    }
    Ok(())
}

fn unique_index(
    index: &ExactRootNameIndexV1<'_>,
    wanted: &str,
    budget: &mut Budget<'_>,
) -> R<usize> {
    match index.find(wanted, budget)? {
        RootNameMatchV1::Unique(ordinal) => Ok(ordinal),
        RootNameMatchV1::Missing => Err(E::Mismatch("complete final receipt root join")),
        RootNameMatchV1::Duplicate => Err(E::Mismatch("unique final receipt root join")),
    }
}

pub(super) fn reports<'a>(owner: OutputOwnerV1<'a>) -> R<&'a [FormalMemoryObligations]> {
    match owner {
        OutputOwnerV1::Direct6(owner) => Ok(owner.kernels()),
        OutputOwnerV1::Erased6(owner) => Ok(owner.kernels()),
        _ => Err(E::Mismatch("fixed Policy6 receipt owner")),
    }
}

pub(super) fn check(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    receipts: &NativeFinalOutputReceiptsPolicy6V1,
    typed: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> R<()> {
    check_fixed(inputs, ranked, receipts, typed, FixedOutput::I, budget)
}

/// Fixed Policy7 entry: report custody comes only from the actual J owner.
pub(in crate::production_pipeline) fn check_policy7(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    receipts: &NativeFinalOutputReceiptsPolicy6V1,
    typed: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> R<()> {
    check_fixed(inputs, ranked, receipts, typed, FixedOutput::J, budget)
}

/// Fixed Policy8 entry accepts only actual K owner/report custody.
pub(in crate::production_pipeline) fn check_policy8(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    receipts: &NativeFinalOutputReceiptsPolicy6V1,
    typed: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> R<()> {
    check_fixed(inputs, ranked, receipts, typed, FixedOutput::K, budget)
}

enum FixedOutput {
    I,
    J,
    K,
}

fn check_fixed(
    inputs: OutputInputsV1<'_>,
    ranked: &Ranked,
    receipts: &NativeFinalOutputReceiptsPolicy6V1,
    typed: &[TypedDescriptorRootV1],
    fixed: FixedOutput,
    budget: &mut Budget<'_>,
) -> R<()> {
    scoped(budget, |budget| {
        budget.charge_work(8)?;
        if budget.storage() < receipts.retained {
            return Err(Resource::Accounting.into());
        }
        let output = inputs.owner.output();
        let source = inputs.owner.source(inputs.catalog)?;
        let (reports, receipt_error, payload_error) = match fixed {
            FixedOutput::K => (
                match inputs.owner {
                    OutputOwnerV1::Direct8(owner) => owner.kernels(),
                    OutputOwnerV1::Erased8(owner) => owner.kernels(),
                    _ => return Err(E::Mismatch("fixed Policy8 receipt owner")),
                },
                "fresh final-K formal receipt",
                "fresh final-K formal payload",
            ),
            FixedOutput::I => (
                reports(inputs.owner)?,
                "fresh final-I formal receipt",
                "fresh final-I formal payload",
            ),
            FixedOutput::J => (
                match inputs.owner {
                    OutputOwnerV1::Direct7(owner) => owner.kernels(),
                    OutputOwnerV1::Erased7(owner) => owner.kernels(),
                    _ => return Err(E::Mismatch("fixed Policy7 receipt owner")),
                },
                "fresh final-J formal receipt",
                "fresh final-J formal payload",
            ),
        };
        let wire = receipts.kernel_ir.canonical_preimage();
        budget.charge_work(112)?;
        let envelope = NativeNeutralModuleRefV1::decode(wire)
            .map_err(|_| E::Mismatch("final KernelIr native envelope"))?;
        let expected = subject(output, inputs.catalog)?;
        if envelope.subject() != &expected {
            return Err(E::Mismatch("final KernelIr subject"));
        }
        same_bytes(
            envelope.graph_bytes(),
            output.canonical().canonical_bytes(),
            budget,
            "final KernelIr graph bytes",
        )?;
        same_bytes(
            envelope.catalog_bytes(),
            inputs.catalog.canonical_bytes(),
            budget,
            "final KernelIr catalog bytes",
        )?;

        let wire = receipts.formal_memory.canonical_preimage();
        let wire_floor = wire
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<Roster>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wire_floor)?;
        budget.charge_work(wire.len())?;
        // Existing roster metadata and formal codec internals retain their own
        // capped domains. Here we prepay visible decoded headers/wire and joins.
        let roster =
            Roster::decode(wire).map_err(|_| E::Mismatch("final FormalMemory native roster"))?;
        let count = ranked.root_count();
        let (_, descriptors, _) = inputs.prepared.native_output_parts_v1();
        let descriptor_rows = descriptors.table().kernels();
        if !(1..=MAX_ROOTS).contains(&count)
            || roster.kind() != Kind::FormalMemory
            || roster.root_count() != count
            || roster.native_neutral_subject() != &expected
            || roster.semantic_mir_sha256() != *source.semantic.semantic_sha256().as_bytes()
            || roster.roster_identity() != *ranked.canonical_roster_identity().as_bytes()
            || source.semantic.roots().len() != count
            || source.launch.roots().len() != count
            || source.original.module().kernels.len() != count
            || output.module().kernels.len() != count
            || reports.len() != count
            || typed.len() != count
            || descriptor_rows.len() != count
            || ranked.canonical_kernel_order().len() != count
        {
            return Err(E::Mismatch("complete final FormalMemory roster"));
        }
        for (actual, expected) in roster
            .canonical_kernel_order()
            .iter()
            .zip(ranked.canonical_kernel_order())
        {
            budget.charge_work(2)?;
            if usize::try_from(*actual).ok() != Some(*expected) {
                return Err(E::Mismatch("final receipt descriptor-canonical order"));
            }
        }

        budget.reserve_storage(MAX_ROOTS.checked_mul(3).ok_or(Resource::Arithmetic)?)?;
        let mut input_seen = [false; MAX_ROOTS];
        let mut output_seen = [false; MAX_ROOTS];
        let mut descriptor_seen = [false; MAX_ROOTS];
        let input_names = ExactRootNameIndexV1::build(
            source
                .original
                .module()
                .kernels
                .iter()
                .map(|k| k.id.as_str()),
            budget,
        )?;
        let output_names = ExactRootNameIndexV1::build(
            output.module().kernels.iter().map(|k| k.id.as_str()),
            budget,
        )?;
        let descriptor_names = ExactRootNameIndexV1::build(
            descriptor_rows.iter().map(|k| k.entry_name().as_str()),
            budget,
        )?;
        let typed_names =
            ExactRootNameIndexV1::build(typed.iter().map(|k| k.entry_symbol()), budget)?;
        for (ordinal, root) in ranked.roots().iter().enumerate() {
            budget.charge_work(160)?;
            let row = roster
                .root(ordinal)
                .ok_or(E::Mismatch("final formal semantic root"))?;
            let launch = source.launch.roots()[ordinal];
            let export = std::str::from_utf8(root.export_symbol())
                .map_err(|_| E::Mismatch("final root export encoding"))?;
            let input_index = unique_index(&input_names, export, budget)?;
            let output_index = unique_index(&output_names, export, budget)?;
            let descriptor_index = unique_index(&descriptor_names, export, budget)?;
            let typed_index = unique_index(&typed_names, export, budget)?;
            let actual = &output.module().kernels[output_index];
            let original = &source.original.module().kernels[input_index];
            let report = &reports[output_index];
            let typed_root = &typed[typed_index];
            if input_seen[input_index]
                || output_seen[output_index]
                || descriptor_seen[descriptor_index]
                || source.semantic.roots()[ordinal] != root.semantic_root()
                || launch.selected_root() != root.semantic_root()
                || launch.semantic_root_identity() != root.semantic_root_identity()
                || launch.kernel_binding() != *root.kernel_binding()
                || row.semantic_root() != root.semantic_root().index()
                || row.semantic_root_identity() != *root.semantic_root_identity().as_bytes()
                || row.kernel_binding() != *root.kernel_binding()
                || row.source_rank() != root.source_rank()
                || row.source_rank() != launch.source_rank()
                || Some(row.workgroup()) != launch.source_launch().exact_workgroup()
                || row.logical_name() != root.logical_name()
                || row.export_symbol() != export
                || row.kernel_id() != actual.id.as_str()
                || typed_root.logical_name() != root.logical_name()
                || typed_root.kernel_binding_bytes() != *root.kernel_binding()
                || original.entry != actual.entry
                || report.kernel() != &actual.id
                || report.entry() != &actual.entry
                || roster
                    .canonical_kernel_order()
                    .get(descriptor_index)
                    .copied()
                    != u32::try_from(ordinal).ok()
            {
                return Err(E::Mismatch("exact final receipt root axes"));
            }
            input_seen[input_index] = true;
            output_seen[output_index] = true;
            descriptor_seen[descriptor_index] = true;
            // The fixed entry selects actual fresh final-graph obligations,
            // never original N or any historical optimization-stage reports.
            let temporary = MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1
                .checked_add(size_of::<Formal>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(temporary)?;
            let expected =
                Formal::from_current_obligations(report).map_err(|_| E::Mismatch(receipt_error))?;
            same_bytes(
                row.payload(),
                expected.canonical_bytes(),
                budget,
                payload_error,
            )?;
            drop(expected);
            budget.release_storage(temporary)?;
        }
        drop(roster);
        Ok(())
    })
}

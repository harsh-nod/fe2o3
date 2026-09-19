//! Exact N/root/formal joins, never a caller-selected original/final mode.
use super::super::root_index::{ExactRootNameIndexV1, RootNameMatchV1};
use super::*;

fn unique_index(
    index: &ExactRootNameIndexV1<'_>,
    wanted: &str,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    match index.find(wanted, budget)? {
        RootNameMatchV1::Unique(ordinal) => Ok(ordinal),
        RootNameMatchV1::Missing => Err(Error::Mismatch("complete original receipt root join")),
        RootNameMatchV1::Duplicate => Err(Error::Mismatch("unique original receipt root join")),
    }
}

pub(super) fn check_roots(
    source: SourceInputsV1<'_>,
    ranked: &Ranked,
    typed: &[TypedDescriptorRootV1],
    reports: &OriginalNativeFormalMemoryV1<'_>,
    expected: &Subject,
    roster: &Roster,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let count = ranked.root_count();
    budget.charge_work(12)?;
    if !(1..=MAX_ROOTS).contains(&count)
        || roster.kind() != Kind::FormalMemory
        || roster.root_count() != count
        || roster.native_neutral_subject() != expected
        || roster.semantic_mir_sha256() != *source.semantic.semantic_sha256().as_bytes()
        || roster.roster_identity() != *ranked.canonical_roster_identity().as_bytes()
        || source.semantic.roots().len() != count
        || source.launch.roots().len() != count
        || source.original.module().kernels.len() != count
        || reports.kernels().len() != count
        || typed.len() != count
        || ranked.canonical_kernel_order().len() != count
    {
        return Err(Error::Mismatch("complete original FormalMemory roster"));
    }
    for (actual, expected) in roster
        .canonical_kernel_order()
        .iter()
        .zip(ranked.canonical_kernel_order())
    {
        budget.charge_work(2)?;
        if usize::try_from(*actual).ok() != Some(*expected) {
            return Err(Error::Mismatch(
                "original receipt descriptor-canonical order",
            ));
        }
    }
    budget.reserve_storage(MAX_ROOTS.checked_mul(2).ok_or(Resource::Arithmetic)?)?;
    let mut source_seen = [false; MAX_ROOTS];
    let mut typed_seen = [false; MAX_ROOTS];
    let source_index = ExactRootNameIndexV1::build(
        source
            .original
            .module()
            .kernels
            .iter()
            .map(|k| k.id.as_str()),
        budget,
    )?;
    let typed_index = ExactRootNameIndexV1::build(typed.iter().map(|k| k.entry_symbol()), budget)?;
    for (ordinal, root) in ranked.roots().iter().enumerate() {
        budget.charge_work(120)?;
        let row = roster
            .root(ordinal)
            .ok_or(Error::Mismatch("original formal semantic root"))?;
        let export = std::str::from_utf8(root.export_symbol())
            .map_err(|_| Error::Mismatch("original root export encoding"))?;
        let index = unique_index(&source_index, export, budget)?;
        let typed_ordinal = unique_index(&typed_index, export, budget)?;
        let original = &source.original.module().kernels[index];
        let report = &reports.kernels()[index];
        let launch = source.launch.roots()[ordinal];
        let descriptor = &typed[typed_ordinal];
        if source_seen[index]
            || typed_seen[typed_ordinal]
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
            || row.kernel_id() != original.id.as_str()
            || descriptor.logical_name() != root.logical_name()
            || descriptor.kernel_binding_bytes() != *root.kernel_binding()
            || report.kernel() != &original.id
            || report.entry() != &original.entry
        {
            return Err(Error::Mismatch("exact original receipt root axes"));
        }
        source_seen[index] = true;
        typed_seen[typed_ordinal] = true;
        let temporary = MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1
            .checked_add(size_of::<Formal>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(temporary)?;
        let fresh = Formal::from_current_obligations(report)
            .map_err(|_| Error::Mismatch("fresh original-N formal receipt"))?;
        same_bytes(
            row.payload(),
            fresh.canonical_bytes(),
            budget,
            "fresh original-N formal payload",
        )?;
        drop(fresh);
        budget.release_storage(temporary)?;
    }
    Ok(())
}

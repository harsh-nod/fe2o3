use super::*;
use fe2o3_kernel_descriptor::{BlockSizeV1, MAX_KERNELS};

pub(super) fn check(
    request: &Request<'_>,
    (root_id, body_id): (SemanticFunctionIdV1, SemanticFunctionIdV1),
    expected: KernelId,
    table: &Table<'_>,
    descriptor: &Kernel<'_, '_>,
    budget: &mut Budget<'_>,
) -> R<()> {
    let owner = request.source();
    let semantic = owner.semantic_ssa().source_semantic();
    let launch = owner.source_launch();
    let graph = owner.executable().module();
    budget.charge_work(8)?;
    require(
        matches!(
            semantic.target().architecture(),
            fe2o3_mir_model::semantic_mir_v1::SemanticTargetArchitectureV1::AmdGpuGfx942
        ),
        "supported 64-bit source ABI",
    )?;
    // This semantic architecture describes the shared ABI, not the exact CPU
    // or native feature roster. The outer target-aware F/V5 text join owns that.
    require(
        (1..=MAX_KERNELS).contains(&semantic.roots().len())
            && table.kernel_count() == semantic.roots().len()
            && launch.roots().len() == semantic.roots().len()
            && graph.kernels.len() == semantic.roots().len(),
        "complete source/descriptor roster sizes",
    )?;
    budget.charge_work(sum(&[semantic.roots().len(), 64])?)?;
    require(
        launch.semantic_sha256() == semantic.semantic_sha256().as_bytes(),
        "launch source identity",
    )?;
    let mut ordinal = None;
    for (i, id) in semantic.roots().iter().enumerate() {
        if *id == root_id && ordinal.replace(i).is_some() {
            return Err(E::Mismatch("unique source root"));
        }
    }
    let ordinal = ordinal.ok_or(E::Mismatch("source root membership"))?;
    let root = semantic
        .functions()
        .get(root_id.index() as usize)
        .ok_or(E::Mismatch("source root"))?;
    require(
        semantic.functions().get(body_id.index() as usize).is_some(),
        "source body",
    )?;
    let entry = root
        .kernel_entry()
        .ok_or(E::Mismatch("physical kernel entry"))?;
    let actual_launch = launch.roots()[ordinal];
    let symbol = entry.export_symbol().as_bytes();
    let recipe = request.pliron_input().kernel().function_name();
    budget.charge_work(sum(&[
        symbol.len(),
        recipe.len(),
        descriptor.entry_name().len(),
        descriptor.descriptor_symbol().len(),
        128,
    ])?)?;
    require(
        expected.as_bytes() == entry.kernel_binding_identity().as_bytes()
            && descriptor.kernel_id() == expected
            && descriptor.entry_name().as_bytes() == symbol
            && descriptor
                .descriptor_symbol()
                .strip_suffix(".kd")
                .map(str::as_bytes)
                == Some(symbol)
            && recipe.as_bytes() == symbol
            && actual_launch.selected_root() == root_id
            && actual_launch.semantic_root_identity() == root.identity()
            && actual_launch.kernel_binding() == *expected.as_bytes(),
        "exact source/recipe/descriptor root",
    )?;
    let pending = request
        .translation()
        .pending()
        .kernel()
        .map_err(|e| E::Aggregate(ProductionConditionalAggregateErrorV1::Session(e)))?;
    require(
        std::ptr::eq(request.translation().source(), owner)
            && std::ptr::eq(pending, request.pliron_input().kernel()),
        "actual source/pending graph borrow",
    )?;
    let mut selected = None;
    for kernel in &graph.kernels {
        budget.charge_work(sum(&[kernel.id.as_str().len(), symbol.len(), 1])?)?;
        if kernel.id.as_str().as_bytes() == symbol && selected.replace(kernel).is_some() {
            return Err(E::Mismatch("unique canonical root"));
        }
    }
    let kernel = selected.ok_or(E::Mismatch("canonical root"))?;
    let workgroup = actual_launch
        .source_launch()
        .exact_workgroup()
        .ok_or(E::Mismatch("exact workgroup"))?;
    let BlockSizeV1::Exact(block) = descriptor.launch().block_size() else {
        return Err(E::Mismatch("descriptor workgroup"));
    };
    let grid = descriptor.launch().max_grid();
    let resources = entry
        .source_contract()
        .resources()
        .map(|r| {
            (
                r.static_shared_memory_bytes(),
                r.max_dynamic_shared_memory_bytes(),
            )
        })
        .unwrap_or_default();
    budget.charge_work(32)?;
    require(
        actual_launch.source_rank() == 1
            && kernel.domain.rank() == 1
            && descriptor.launch().rank() == 1
            && kernel.workgroup_size.map(|w| [w.x, w.y, w.z]) == Some(workgroup)
            && [block.x(), block.y(), block.z()] == workgroup
            && [grid.x(), grid.y(), grid.z()] == actual_launch.source_launch().max_grid()
            && descriptor.launch().max_flat_workgroup_size()
                == workgroup
                    .into_iter()
                    .try_fold(1u32, u32::checked_mul)
                    .ok_or(Resource::Arithmetic)?
            && (
                descriptor.launch().static_shared_memory_bytes(),
                descriptor.launch().max_dynamic_shared_memory_bytes(),
            ) == resources,
        "exact source/N/descriptor launch",
    )
}

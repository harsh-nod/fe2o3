//! Exact source/I/J observations; production replay owns the source-origin proof.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as Coordinate, FormalMemoryObligations,
    InertFormalMemoryReceiptFormatV4 as Formal, LaunchDomain, LaunchExtent, Module, Operation,
    OperationKind as Kind, ScalarType, Type, WorkgroupSize,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observed {
    pub(super) source: Source,
    pub(super) original: [u8; 32],
    pub(super) input: [u8; 32],
    pub(super) output: [u8; 32],
    source_pair: [u32; 5],
    pub(super) deletions: usize,
    retained: usize,
    input_operations: usize,
    output_operations: usize,
    private_before: usize,
    private_after: usize,
    global_before: usize,
    global_after: usize,
    private_loads: usize,
    moved_global_accesses: usize,
    historical_formal: [u8; 32],
    final_formal: [u8; 32],
    execution: [u8; 32],
    replay_work: usize,
    pub(super) llvm_sha256: [u8; 32],
    pub(super) llvm_bytes: usize,
    pub(super) sim: sim::Report,
}

fn u32_place(semantic: &AdmittedInertSemanticMirV1, place: &SemanticPlaceV1) -> bool {
    place.projections().is_empty()
        && matches!(
            semantic.types()[place.ty().index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32
            })
        )
}
fn destination(kind: &SemanticStatementKindV1) -> Option<&SemanticPlaceV1> {
    match kind {
        SemanticStatementKindV1::Assign(value) => Some(value.destination()),
        SemanticStatementKindV1::Store(value) => Some(value.destination()),
        _ => None,
    }
}
fn source_pair(
    semantic: &AdmittedInertSemanticMirV1,
    active: active_source::ActiveSource,
) -> Result<[u32; 5], String> {
    let [root] = semantic.roots() else {
        return Err("one semantic smoke root".into());
    };
    let selected = semantic
        .select_kernel_body_for_root_v1(*root)
        .ok_or("actual selected source body")?;
    if selected.root() != *root {
        return Err("foreign selected body".into());
    }
    let body = &semantic.functions()[selected.body().index() as usize];
    let mut candidates = Vec::new();
    for (block_index, block) in body.blocks().iter().enumerate() {
        for (borrow_index, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place,
            } = assignment.value().kind()
            else {
                continue;
            };
            if !u32_place(semantic, place) {
                continue;
            }
            let writes = block.statements()[..borrow_index]
                .iter()
                .enumerate()
                .filter(|(_, statement)| destination(statement.kind()) == Some(place))
                .collect::<Vec<_>>();
            let [(first, first_statement), (second, second_statement)] = writes.as_slice() else {
                continue;
            };
            for statement in [*first_statement, *second_statement] {
                let source = statement.source();
                let origin = source
                    .call_site()
                    .or_else(|| source.expansion())
                    .ok_or("candidate source span missing")?;
                active.check_origin(&origin)?;
            }
            for statement in &block.statements()[*first..=*second] {
                let invalid = match statement.kind() {
                    SemanticStatementKindV1::StorageLive(local)
                    | SemanticStatementKindV1::StorageDead(local) => *local == place.local(),
                    SemanticStatementKindV1::Deinitialize(killed) => {
                        killed.local() == place.local()
                    }
                    _ => false,
                };
                if invalid {
                    return Err("source pair crosses an inclusive lifetime kill".into());
                }
                let mut moved = false;
                let mut inspect = |operand: &SemanticOperandV1| {
                    moved |= matches!(operand, SemanticOperandV1::Move(moved) if moved.local() == place.local());
                    Ok::<_, String>(())
                };
                match statement.kind() {
                    SemanticStatementKindV1::Assign(value) => {
                        value.value().kind().try_visit_operands(&mut inspect)?
                    }
                    SemanticStatementKindV1::Store(value) => inspect(value.value())?,
                    SemanticStatementKindV1::Assume(value) => inspect(value)?,
                    _ => {}
                }
                if moved {
                    return Err("source pair moves the private destination".into());
                }
            }
            candidates.push([
                block_index as u32,
                *first as u32,
                *second as u32,
                borrow_index as u32,
                place.local().index(),
            ]);
        }
    }
    let [pair] = candidates.as_slice() else {
        return Err(format!(
            "expected one actual source pair followed by mutable borrow, found {}",
            candidates.len()
        ));
    };
    Ok(*pair)
}
fn operations(module: &Module) -> BTreeMap<Coordinate, &Operation> {
    let mut out = BTreeMap::new();
    for (function, value) in module.functions.iter().enumerate() {
        if let Some(body) = &value.body {
            for (block, value) in body.blocks.iter().enumerate() {
                for (operation, value) in value.operations.iter().enumerate() {
                    out.insert(
                        Coordinate {
                            block: Block {
                                function: FunctionCoordinate(function as u32),
                                block: block as u32,
                            },
                            operation: operation as u32,
                        },
                        value,
                    );
                }
            }
        }
    }
    out
}
fn root(module: &Module) -> Result<usize, String> {
    let [kernel] = module.kernels.as_slice() else {
        return Err("exact I/J single root".into());
    };
    let function = module
        .function(&kernel.entry)
        .ok_or("actual kernel entry")?;
    let [Type::Slice(output), input] = function.signature.parameters.as_slice() else {
        return Err("exact output/u32 source ABI".into());
    };
    if kernel.id.as_str() != ROOT
        || output.address_space != AddressSpace::Global
        || output.access != AccessMode::ReadWrite
        || output.element.as_ref() != &Type::Scalar(ScalarType::U32)
        || input != &Type::Scalar(ScalarType::U32)
        || !function.signature.results.is_empty()
        || !matches!(
            kernel.domain,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic
            }
        )
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
    {
        return Err("exact source-derived ABI, root or dynamic64 launch changed".into());
    }
    module
        .functions
        .iter()
        .position(|value| std::ptr::eq(value, function))
        .ok_or_else(|| "actual physical function".to_owned())
}
fn counts(operations: &BTreeMap<Coordinate, &Operation>) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for operation in operations.values() {
        match &operation.kind {
            Kind::Store { access, .. } if access.address_space == AddressSpace::Private => {
                counts.0 += 1
            }
            Kind::Store { access, .. } if access.address_space == AddressSpace::Global => {
                counts.1 += 1
            }
            Kind::Load { access, .. } if access.address_space == AddressSpace::Private => {
                counts.2 += 1
            }
            _ => {}
        }
    }
    counts
}
fn formal(reports: &[FormalMemoryObligations]) -> Result<Vec<u8>, String> {
    let [report] = reports else {
        return Err("one actual formal report".into());
    };
    if report.accesses().is_empty() {
        return Err("source smoke must retain external accesses".into());
    }
    Formal::from_current_obligations(report)
        .map(|v| v.canonical_bytes().to_vec())
        .map_err(|e| format!("{e:?}"))
}

pub(super) fn observe(
    view: source_observation::View<'_>,
    active: active_source::ActiveSource,
    budget: &mut Budget<'_>,
) -> Result<Observed, String> {
    let owner = view
        .direct
        .ok_or("first ordinary-source smoke requires actual Direct route")?;
    let artifacts = view.artifacts;
    let semantic = owner.prefix().source_semantic_kir().semantic().semantic();
    let source = source(semantic)?;
    let source_pair = source_pair(semantic, active)?;
    let i = owner.prefix().output();
    let j = owner.output();
    if !std::ptr::eq(j, artifacts.output())
        || owner.grants_artifact_or_launch_authority()
        || artifacts.grants_artifact_or_launch_authority()
        || owner.continuation().grants_authority()
        || artifacts.execution().policy_version() != 7
        || i.canonical().identity() == j.canonical().identity()
    {
        return Err("actual source/I/J custody or mandatory mutation missing".into());
    }
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let before_work = budget.work();
    // This replays the real retained-origin/source-site/lifetime join and fresh
    // J admission, rather than reconstructing a correspondence in the test.
    view.replay(budget)
        .map_err(|e| format!("source/I/J/native/witness replay: {e}"))?;
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err("actual continuation replay changed the live ledger/floor".into());
    }
    let replay_work = budget.work() - before_work;
    let function = root(i.module())?;
    root(artifacts.original().module())?;
    if root(j.module())? != function
        || artifacts.original().module().kernels[0].entry != i.module().kernels[0].entry
        || i.module().kernels[0].entry != j.module().kernels[0].entry
    {
        return Err("actual N/I/J root function changed".into());
    }
    let before = operations(i.module());
    let after = operations(j.module());
    let rows = owner.continuation().rows();
    if rows.len() != 1 {
        return Err(format!(
            "expected the one genuine I pair deletion, got {}",
            rows.len()
        ));
    }
    let row = rows[0];
    if row.anchor.block != row.removed.block
        || row.anchor.operation >= row.removed.operation
        || row.anchor.block.function.0 as usize != function
    {
        return Err("actual pair is not ordered in the root's one block".into());
    }
    let anchor = *before.get(&row.anchor).ok_or("actual I anchor missing")?;
    let removed = *before.get(&row.removed).ok_or("actual I removal missing")?;
    let Kind::Store {
        pointer,
        value: _,
        access,
    } = &anchor.kind
    else {
        return Err("actual I anchor is not Store".into());
    };
    if anchor != removed
        || access.address_space != AddressSpace::Private
        || access.alignment != 4
        || access.volatile
    {
        return Err(
            "actual I stores differ in pointer/value/access or are not aligned private u32".into(),
        );
    }
    let allocations = before.iter().filter(|(coordinate, operation)| coordinate.block.function == row.anchor.block.function
        && matches!(&operation.kind, Kind::Alloca { element, count: None, address_space: AddressSpace::Private, alignment: 4 } if element == &Type::Scalar(ScalarType::U32))
        && matches!(operation.results.as_slice(), [result] if result.id == *pointer)).count();
    if allocations != 1 {
        return Err("actual I pair lacks its unique direct scalar Alloca".into());
    }
    let retained = owner.continuation().retained_operations();
    let mut inputs = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    let mut moved_global_accesses = 0;
    for retained in retained {
        if !inputs.insert(retained.input)
            || !outputs.insert(retained.output)
            || before.get(&retained.input) != after.get(&retained.output)
        {
            return Err("actual retained rows are not an exact operation join".into());
        }
        if retained.input.block == retained.output.block
            && retained.input.operation > retained.output.operation
            && matches!(&before[&retained.input].kind, Kind::Store { access, .. } if access.address_space == AddressSpace::Global)
        {
            moved_global_accesses += 1;
        }
    }
    if !inputs.contains(&row.anchor)
        || inputs.contains(&row.removed)
        || !inputs.insert(row.removed)
        || inputs != before.keys().copied().collect()
        || outputs != after.keys().copied().collect()
    {
        return Err("complete I/J operation coverage or first-Store retention failed".into());
    }
    let old_formal = formal(owner.prefix().kernels())?;
    let new_formal = formal(owner.kernels())?;
    if old_formal == new_formal || moved_global_accesses == 0 {
        return Err("the real later global access did not move to fresh J coordinates".into());
    }
    let descriptor = artifacts.descriptor_source();
    let [descriptor_root] = descriptor.table().kernels() else {
        return Err("one actual descriptor root".into());
    };
    let entry = semantic.functions()[semantic.roots()[0].index() as usize]
        .kernel_entry()
        .ok_or("actual semantic binding")?;
    let fe2o3_kernel_descriptor::BlockSizeV1::Exact(size) = descriptor_root.launch().block_size()
    else {
        return Err("exact descriptor launch".into());
    };
    if descriptor_root.entry_name().as_str() != ROOT
        || descriptor_root.kernel_id().as_bytes() != entry.kernel_binding_identity().as_bytes()
        || descriptor.table().producer().version().as_str()
            != "production-policy7-checked-gfx942-cov6-v1"
        || [size.x(), size.y(), size.z()] != [64, 1, 1]
        || descriptor_root.launch().rank() != 1
        || descriptor_root.launch().max_flat_workgroup_size() != 64
    {
        return Err("actual J descriptor/source binding or launch changed".into());
    }
    let storage = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES * 3;
    budget
        .reserve_storage(storage)
        .map_err(|e| format!("{e:?}"))?;
    {
        let llvm = dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(j).map_err(|e| format!("{e:?}"))?;
        let llvm = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&llvm)
            .map_err(|e| format!("{e:?}"))?;
        let text =
            crate::kernel_ir_codegen::retain_production_compiler_module_text_v1(j.module(), llvm)
                .map_err(|e| format!("{e:?}"))?;
        let text = crate::kernel_ir_codegen::bind_compiler_descriptor_source_v1(text, descriptor)
            .map_err(|e| format!("{e:?}"))?;
        if text.llvm_ir() != artifacts.llvm_ir() {
            return Err("independent native lowering did not consume exact J".into());
        }
    }
    budget
        .release_storage(storage)
        .map_err(|e| format!("{e:?}"))?;
    let (private_before, global_before, private_loads) = counts(&before);
    let (private_after, global_after, after_loads) = counts(&after);
    if private_loads != after_loads {
        return Err("Store deletion changed private Load count".into());
    }
    let report = Observed {
        source,
        original: *artifacts.original().canonical().identity().digest(),
        input: *i.canonical().identity().digest(),
        output: *j.canonical().identity().digest(),
        source_pair,
        deletions: rows.len(),
        retained: retained.len(),
        input_operations: before.len(),
        output_operations: after.len(),
        private_before,
        private_after,
        global_before,
        global_after,
        private_loads,
        moved_global_accesses,
        historical_formal: digest(&old_formal),
        final_formal: digest(&new_formal),
        execution: digest(artifacts.execution().canonical_bytes()),
        replay_work,
        llvm_sha256: digest(artifacts.llvm_ir().as_bytes()),
        llvm_bytes: artifacts.llvm_ir().len(),
        sim: sim::observe(j.canonical())?,
    };
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err("observation changed live accounting".into());
    }
    validate(&report)?;
    Ok(report)
}
pub(super) fn validate(report: &Observed) -> Result<(), String> {
    if report.source.roots.len() != 1
        || report.source.roots[0].name != ROOT
        || report.source_pair[1] >= report.source_pair[2]
        || report.source_pair[2] >= report.source_pair[3]
        || report.input == report.output
        || report.deletions != 1
        || report.private_before != 2
        || report.private_after != 1
        || report.global_before != 2
        || report.global_after != 2
        || report.private_loads != 1
        || report.retained != report.output_operations
        || report.input_operations != report.output_operations + 1
        || report.moved_global_accesses == 0
        || report.historical_formal == report.final_formal
        || report.replay_work == 0
        || report.llvm_bytes == 0
        || report.execution == [0; 32]
    {
        return Err("mandatory actual source/I/J mutation evidence incomplete".into());
    }
    sim::validate(&report.sim, report.output)
}

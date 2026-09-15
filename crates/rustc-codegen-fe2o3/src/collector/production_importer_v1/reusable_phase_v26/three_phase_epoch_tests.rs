//! Only the negative clones mutate KIR; the positive module is source-emitted.
use super::occurrences::ExpectedPhase;
use fe2o3_kernel_ir::{
    DiagnosticCode, ExecutionCapabilityOpV1, ExecutionCapabilityOperationV1 as Exec,
    ExecutionCapabilityRoleV1, ExecutionMemoryOrderingV1, ExecutionMemoryScopeV1,
    ExecutionMemorySpacesV1, Module, OperationKind, PhaseKeyV1, PhaseOperationSourceV1 as Source,
    ReusablePhaseOperationV1 as Phase, Type, ValueId, verify_module,
};
use fe2o3_lower_mir_kernel::SemanticKirCorrespondenceV1;
use sha2::{Digest as _, Sha256};

struct Row<'a> {
    owner_input: ValueId,
    storage_input: ValueId,
    restored_owner: ValueId,
    restored_storage: ValueId,
    barrier_block: usize,
    barrier_ordinal: usize,
    barrier: &'a ExecutionCapabilityOpV1,
}

fn scoped(key: PhaseKeyV1, marker: [u8; 32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.kir.reusable-phase.source-epoch.v1\0");
    digest.update(key.bytes());
    digest.update(marker);
    digest.finalize().into()
}

pub(super) fn check(
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    expected: &[ExpectedPhase],
) {
    assert_eq!(expected.len(), 3);
    verify_module(module).unwrap();
    let functions = module
        .functions
        .iter()
        .enumerate()
        .filter_map(|(index, function)| {
            let body = function.body.as_ref()?;
            body.blocks
                .iter()
                .flat_map(|b| &b.operations)
                .any(|op| matches!(op.kind, OperationKind::ReusablePhase(_)))
                .then_some((index, body))
        })
        .collect::<Vec<_>>();
    assert_eq!(functions.len(), 1);
    let (function_index, body) = functions[0];
    let operations = body
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(block, b)| {
            b.operations
                .iter()
                .enumerate()
                .map(move |(ordinal, op)| (block, ordinal, op))
        })
        .collect::<Vec<_>>();
    let mut counts = [0; 8];
    let mut converted_owner = None;
    let mut barriers = 0;
    for (_, _, op) in &operations {
        match &op.kind {
            OperationKind::ReusablePhase(phase) => {
                let index = match phase.operation {
                    Phase::OwnerConvert { .. } => {
                        assert!(converted_owner.replace(op.results[0].id).is_none());
                        0
                    }
                    Phase::Begin { .. } => 1,
                    Phase::Bind { .. } => 2,
                    Phase::Seal { .. } => 3,
                    Phase::RelayClosure { .. } => 4,
                    Phase::RelayDrop { .. } => 5,
                    Phase::CloseStorage { .. } => 6,
                    Phase::End { storage_count: 1 } => 7,
                    Phase::End { .. } => panic!("changed allocation roster"),
                };
                counts[index] += 1;
            }
            OperationKind::ExecutionCapability(cap)
                if matches!(cap.operation, Exec::WorkgroupBarrier { .. }) =>
            {
                barriers += 1
            }
            _ => {}
        }
    }
    assert_eq!(counts, [1, 3, 3, 3, 3, 3, 3, 3]);
    assert_eq!(barriers, 3);
    let mut rows = Vec::new();
    let mut keys = std::collections::BTreeSet::new();
    let mut epochs = std::collections::BTreeSet::new();
    for original in expected {
        let key = PhaseKeyV1::for_begin(original.issue).unwrap();
        assert!(keys.insert(key));
        let before = scoped(key, original.initial_marker);
        let after = scoped(key, original.advanced_marker);
        assert!(epochs.insert(before));
        assert!(epochs.insert(after));
        assert_ne!(before, original.initial_marker);
        let begins = operations
            .iter()
            .filter_map(|(_, _, op)| match &op.kind {
                OperationKind::ReusablePhase(p) if matches!(p.operation, Phase::Begin { .. }) => {
                    let Source::Defined(defined) = &p.source else {
                        panic!("defined Begin source");
                    };
                    (defined.call == original.issue).then_some((*op, p, defined))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(begins.len(), 1, "one exact original Issue occurrence");
        let (begin_op, begin, defined) = begins[0];
        assert_eq!(defined.source_binding, original.source_binding);
        let Phase::Begin { dynamic_epoch, .. } = begin.operation else {
            unreachable!()
        };
        assert_eq!(dynamic_epoch, before);
        assert_eq!(begin_op.results.len(), 2);
        let workgroup = &begin_op.results[0];
        let Type::ExecutionCapability(input) = &workgroup.ty else {
            panic!("Begin-issued Workgroup");
        };
        assert_eq!(input.source_type.bytes(), original.workgroup_type);
        assert_eq!(input.role, ExecutionCapabilityRoleV1::Workgroup);
        assert_eq!(input.epoch, Some(before));
        let matches = operations
            .iter()
            .filter_map(|(block, ordinal, op)| match &op.kind {
                OperationKind::ExecutionCapability(cap)
                    if cap.source == original.barrier.source =>
                {
                    Some((*block, *ordinal, cap))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "one exact original Finish terminal");
        let (barrier_block, barrier_ordinal, barrier) = matches[0];
        assert_eq!(barrier.operands.as_slice(), &[workgroup.id]);
        assert_eq!(barrier.provenance, input.provenance);
        assert_eq!(barrier.workgroup_brand, input.workgroup_brand);
        assert_eq!(barrier.epoch_before, Some(before));
        assert_eq!(barrier.epoch_after, Some(after));
        let Exec::WorkgroupBarrier { semantics, .. } = barrier.operation else {
            panic!("Finish barrier");
        };
        assert_eq!(semantics.scope, ExecutionMemoryScopeV1::Workgroup);
        assert_eq!(
            semantics.ordering,
            ExecutionMemoryOrderingV1::AcquireRelease
        );
        assert_eq!(semantics.spaces, ExecutionMemorySpacesV1::Workgroup);
        let spans = correspondence
            .terminator_operation_spans()
            .iter()
            .filter(|span| {
                span.correspondence_owner() == original.root
                    && span.semantic_block().index()
                        == original.barrier.source.occurrence.unwrap().expanded_block()
            })
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 1);
        let span = spans[0];
        assert_eq!(span.kernel_ir_block(), body.blocks[barrier_block].id);
        let first = span.first_operation_ordinal() as usize;
        let end = first.checked_add(span.operation_count() as usize).unwrap();
        assert!((first..end).contains(&barrier_ordinal));
        let seals = operations
            .iter()
            .filter_map(|(block, _, op)| match &op.kind {
                OperationKind::ReusablePhase(p) => match p.operation {
                    Phase::Seal { barrier_call, .. } if barrier_call == original.barrier => {
                        Some(*block)
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            seals.len(),
            1,
            "Seal retains both original and expanded normal targets"
        );
        assert_eq!(
            body.blocks[seals[0]].id.0,
            original.barrier.expanded_normal_target
        );
        let binds = operations
            .iter()
            .filter_map(|(_, _, op)| match &op.kind {
                OperationKind::ReusablePhase(p)
                    if matches!(p.operation, Phase::Bind { .. })
                        && p.operands.get(1) == Some(&workgroup.id) =>
                {
                    Some(p)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(binds.len(), 1);
        let ends = operations
            .iter()
            .filter_map(|(_, _, op)| match &op.kind {
                OperationKind::ReusablePhase(p)
                    if matches!(p.operation, Phase::End { storage_count: 1 })
                        && matches!(p.source, Source::WrapperEnd { phase, .. } if phase == key) =>
                {
                    Some(*op)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(ends.len(), 1);
        assert_eq!(ends[0].results.len(), 2);
        rows.push(Row {
            owner_input: begin.operands[0],
            storage_input: binds[0].operands[2],
            restored_owner: ends[0].results[0].id,
            restored_storage: ends[0].results[1].id,
            barrier_block,
            barrier_ordinal,
            barrier,
        });
    }
    // Source order is witnessed by the actual restored SSA owner, not block IDs.
    let mut next = converted_owner.unwrap();
    let mut ordered: Vec<usize> = Vec::new();
    for _ in 0..3 {
        let matches = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.owner_input == next)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        let index = matches[0];
        assert!(!ordered.contains(&index));
        if let Some(previous) = ordered.last().copied() {
            assert_eq!(rows[index].storage_input, rows[previous].restored_storage);
        }
        ordered.push(index);
        next = rows[index].restored_owner;
    }
    assert!(rows.iter().all(|row| row.owner_input != next));
    let third = &rows[ordered[2]];
    for earlier in ordered[..2].iter().map(|index| &rows[*index]) {
        assert_eq!(earlier.barrier.signature, third.barrier.signature);
        assert_eq!(earlier.barrier.operation, third.barrier.operation);
        assert_eq!(earlier.barrier.provenance, third.barrier.provenance);
        assert_eq!(
            earlier.barrier.workgroup_brand,
            third.barrier.workgroup_brand
        );
        for align_epoch in [false, true] {
            let mut stale = module.clone();
            let changed = &mut stale.functions[function_index]
                .body
                .as_mut()
                .unwrap()
                .blocks[third.barrier_block]
                .operations[third.barrier_ordinal]
                .kind;
            let OperationKind::ExecutionCapability(changed) = changed else {
                unreachable!()
            };
            changed.operands[0] = earlier.barrier.operands[0];
            if align_epoch {
                changed.epoch_before = earlier.barrier.epoch_before;
            }
            assert_eq!(changed.source, third.barrier.source);
            assert_eq!(changed.epoch_after, third.barrier.epoch_after);
            let errors = verify_module(&stale)
                .expect_err("third Finish rejects an earlier phase's actual SSA operand");
            let message = if align_epoch {
                "execution operation consumes an epoch after a dominating transition".into()
            } else {
                format!(
                    "SSA operand 0 ({}) does not match its exact V13 type/role",
                    earlier.barrier.operands[0]
                )
            };
            assert!(
                errors.diagnostics().iter().any(|d| {
                    d.code == DiagnosticCode::InvalidExecutionCapability
                        && d.location.module == module.id
                        && d.location.function.as_ref()
                            == Some(&module.functions[function_index].id)
                        && d.location.block == Some(body.blocks[third.barrier_block].id)
                        && d.location.operation == Some(third.barrier_ordinal)
                        && d.message == message
                }),
                "{errors:?}"
            );
            stale.functions[function_index]
                .body
                .as_mut()
                .unwrap()
                .blocks[third.barrier_block]
                .operations[third.barrier_ordinal]
                .kind = OperationKind::ExecutionCapability(third.barrier.clone());
            assert_eq!(
                &stale, module,
                "no source, barrier, result or other operation was changed"
            );
        }
    }
}

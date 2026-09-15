use super::*;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, Constant, OperationKind, ScalarType, Type};
use std::fmt::Write as _;

fn expect_text(actual: &str, expected: &str) {
    let expected = expected
        .lines()
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(actual.trim_end(), expected.trim_end());
}

fn render_kir(owner: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1) -> String {
    let module = owner.executable().module();
    fe2o3_kernel_ir::verify_module(module).unwrap();
    let [kernel] = module.kernels.as_slice() else {
        panic!("one declared kernel entry is required")
    };
    let body = module
        .function(&kernel.entry)
        .unwrap()
        .body
        .as_ref()
        .unwrap();
    let operations = &body.blocks[0].operations;
    let slots = operations
        .iter()
        .enumerate()
        .filter_map(|(ordinal, operation)| match &operation.kind {
            OperationKind::Alloca {
                element,
                count: Some(count),
                address_space,
                alignment,
            } => {
                assert_eq!(element, &Type::Scalar(ScalarType::U32));
                assert_eq!(*address_space, AddressSpace::Private);
                assert_eq!(*alignment, 4);
                assert_eq!(operation.results.len(), 1);
                assert_eq!(
                    operation.results[0].ty,
                    Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Private,
                        AccessMode::ReadWrite
                    )
                );
                Some((ordinal, operation.results[0].id, *count))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(allocation_ordinal, slot, count)] = slots.as_slice() else {
        panic!("one real counted private u32 allocation is required")
    };
    let index_value = |id| {
        operations
            .iter()
            .find_map(|operation| {
                (operation
                    .results
                    .first()
                    .is_some_and(|result| result.id == id))
                .then_some(&operation.kind)
            })
            .map(|kind| match kind {
                OperationKind::Constant(Constant::Index(value)) => *value,
                other => panic!("the actual index must be a direct Index constant: {other:?}"),
            })
            .unwrap()
    };
    let count = index_value(*count);
    let mut output = format!(
        "KIR alloca={allocation_ordinal} element=U32 space=Private access=ReadWrite align=4 count={count}\n"
    );
    let mut stores = 0;
    for (ordinal, operation) in operations.iter().enumerate() {
        if let OperationKind::Store {
            pointer, access, ..
        } = &operation.kind
        {
            assert_eq!(access.address_space, AddressSpace::Private);
            assert_eq!(access.alignment, 4);
            assert!(!access.volatile);
            let (gep_ordinal, offset) = operations
                .iter()
                .enumerate()
                .find_map(|(index, operation)| {
                    if operation
                        .results
                        .first()
                        .is_some_and(|result| result.id == *pointer)
                        && let OperationKind::GetElementPointer { base, offset } = operation.kind
                    {
                        assert_eq!(base, *slot);
                        assert!(index < ordinal);
                        Some((index, offset))
                    } else {
                        None
                    }
                })
                .unwrap();
            writeln!(output, "KIR store={ordinal} gep={gep_ordinal} index={} space=Private align=4 volatile=false", index_value(offset)).unwrap();
            stores += 1;
        }
    }
    assert_eq!(stores, 2);
    assert!(
        !body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                OperationKind::Load { .. } | OperationKind::GuardedLoad { .. }
            ))
    );
    output.push_str("KIR loads=0 stores=2\n");
    output
}

fn render_queries(
    owner: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    output: &mut String,
) {
    owner.semantic_ssa().verify_replay().unwrap();
    assert_eq!(
        constant_locals(&owner.semantic_ssa().source_semantic().functions()[0]).unwrap()[2],
        None
    );
    with_canonical_assertions_v1(owner, |session| {
        let mut facts = session.for_source(ROOT, ROOT);
        for statement in [1, 3] {
            let index = facts
                .private_array_constant_index(
                    Site::Statement {
                        block: SsaBlockIdV1::new(0),
                        statement,
                    },
                    Role::Destination,
                )?
                .expect("an actual retained-array occurrence index");
            writeln!(
                output,
                "QUERY source=0/{statement}/Destination index={index}"
            )
            .unwrap();
        }
        Ok(())
    })
    .unwrap();
}

fn render_ranked(root: &ProductionRankedRootProgramV1, output: &mut String) {
    assert!(root.lowering.all_mandatory_reports_are_clean());
    assert_eq!(root.access_sources.len(), 2);
    let blocks = root.lowering.kernel().blocks();
    let definition = |value| {
        let ProductionRankedValueV1::Local(value) = value else {
            panic!("local ranked value")
        };
        blocks
            .iter()
            .flat_map(|block| block.operations())
            .find(|operation| match operation {
                ProductionRankedOperationV1::IndexConstant { result, .. }
                | ProductionRankedOperationV1::ViewInSpace { result, .. } => *result == value,
                _ => false,
            })
            .unwrap()
    };
    for row in &root.access_sources {
        let ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        } = &blocks[row.ranked_block() as usize].operations()[row.ranked_operation() as usize]
        else {
            panic!("one ordinary actual ranked access per source row")
        };
        assert_eq!(*kind, AccessKindAttr::Write);
        let ProductionRankedOperationV1::ViewInSpace {
            element_width,
            writable,
            shape,
            memory_space,
            ..
        } = definition(*view)
        else {
            panic!("the actual retained private view")
        };
        assert_eq!(
            (*element_width, *writable, *memory_space),
            (32, true, MemorySpaceAttr::Private)
        );
        assert_eq!(shape.as_slice(), &[8]);
        let [index] = indices.as_slice() else {
            panic!("one array index")
        };
        let ProductionRankedOperationV1::IndexConstant { value, .. } = definition(*index) else {
            panic!("the actual source occurrence must project a constant")
        };
        writeln!(
            output,
            "RANKED source={}/{}/{} Write Private width=32 shape=8 index={value}",
            row.semantic_block(),
            row.semantic_statement().unwrap(),
            row.semantic_access_ordinal()
        )
        .unwrap();
    }
}

#[test]
fn private_array_reassignment_textual_expectation_uses_actual_owner() {
    let owner = assertion_materialized(source(7, Some(0)));
    let canonical = owner.executable().canonical().canonical_bytes().as_ptr();
    let graph = owner.executable().module().functions.as_ptr();
    let mut output = render_kir(&owner);
    render_queries(&owner, &mut output);
    let (owner, root) = split(assertion_project(owner).unwrap());
    render_ranked(&root, &mut output);
    owner.semantic_ssa().verify_replay().unwrap();
    let attached = attach(owner, root).unwrap();
    let retained = attached.pre_ranked_executable().unwrap();
    assert_eq!(retained.module().functions.as_ptr(), graph);
    assert_eq!(retained.canonical().canonical_bytes().as_ptr(), canonical);
    assert!(!attached.grants_artifact_or_launch_authority());
    output.push_str("ATTACH same-graph=true same-canonical=true authority=false\n");
    expect_text(
        &output,
        include_str!("private_array_occurrence_lit/reassignment.check"),
    );
}

#[test]
fn private_array_equal_indices_textual_expectation_keeps_distinct_occurrences() {
    let owner = assertion_materialized(source(0, Some(0)));
    let mut output = render_kir(&owner);
    render_queries(&owner, &mut output);
    let (owner, mut root) = split(assertion_project(owner).unwrap());
    render_ranked(&root, &mut output);
    owner.semantic_ssa().verify_replay().unwrap();
    let first = root.access_sources[0];
    let second = root.access_sources[1];
    root.access_sources[0] = access_row(first, second);
    root.access_sources[1] = access_row(second, first);
    let Err(error) = attach(owner, root) else {
        panic!("equal index values cannot authorize swapped physical source sites")
    };
    assert!(matches!(
        error,
        LoweringError::MirPlironTranslation(Translation::ControlFlowMismatch { .. })
    ));
    output.push_str("ATTACH swapped-physical-sites=ControlFlowMismatch\n");
    expect_text(
        &output,
        include_str!("private_array_occurrence_lit/equal_indices_swapped.check"),
    );
}

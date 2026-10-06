use super::*;
use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;

include!("original_semantic_mir_nominal_aggregate_dispatch_v48_tests.rs");

const LIMIT: usize = 256 << 20;

fn fixture(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
    depth: usize,
) {
    let helper = functions.last_mut().unwrap();
    let source = helper.source();
    let unit = TypeId::from_index(
        types
            .iter()
            .position(|ty| matches!(ty.shape(), Shape::Unit))
            .unwrap() as u32,
    );
    let raw = TypeId::from_index(types.len() as u32);
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256([230; 32]),
        SemanticLayoutIdentityV1::from_sha256([230; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            BackendRepr::scalar(BackendScalar::initialized(
                BackendPrimitive::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        Shape::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    let witness = TypeId::from_index(types.len() as u32);
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256([231; 32]),
        SemanticLayoutIdentityV1::from_sha256([231; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            types[raw.index() as usize].layout().backend_repr().clone(),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        Shape::Aggregate(SemanticAggregateTypeV1::new(vec![raw, unit]).unwrap()),
    ));
    let reference = TypeId::from_index(types.len() as u32);
    types.push(
        Type::new(
            SemanticTypeIdentityV1::from_sha256([232; 32]),
            SemanticLayoutIdentityV1::from_sha256([232; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                BackendRepr::scalar(BackendScalar::initialized(
                    BackendPrimitive::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            Shape::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    witness,
                    PointerKind::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    PointerMetadata::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        8,
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let shared = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        8,
        Some(8),
    )
    .unwrap();
    let issue = callables.len() as u32;
    for (tag, inputs, output, operation) in [
        (
            240,
            vec![],
            witness,
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: witness,
                raw_index: raw,
            },
        ),
        (
            241,
            vec![reference],
            raw,
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                index_witness: witness,
                raw_index: raw,
            },
        ),
    ] {
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            helper.abi().layout_identity(),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            inputs
                .iter()
                .map(|&ty| SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Direct(shared)))
                .collect(),
            SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Direct(plain)),
        )
        .unwrap()
        .with_source_argument_ownership(
            inputs
                .iter()
                .map(|_| SemanticSourceArgumentOwnershipV1::SharedBorrow)
                .collect(),
        )
        .unwrap();
        callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                source,
                abi,
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
        });
    }
    let mut locals = helper.locals().to_vec();
    assert_eq!(locals.len(), 4);
    for ty in [witness, reference, reference, raw]
        .into_iter()
        .chain(std::iter::repeat_n(reference, depth))
    {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([100 + locals.len() as u8; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
    }
    let place = |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |destination: Place, value| {
        SemanticStatementV1::new(
            source,
            Statement::Assign(SemanticAssignmentV1::new(
                destination.clone(),
                SemanticRvalueV1::new(destination.ty(), value),
            )),
        )
    };
    let call = |callee, arguments, local, ty, next| {
        Terminator::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(local, ty),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(next),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let block = |ordinal: u8, statements, term| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([200 + ordinal; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, term),
        )
        .unwrap()
    };
    let mut statements = vec![
        assign(
            place(5, reference),
            Rvalue::Borrow {
                kind: BorrowKind::Shared,
                place: place(4, witness),
            },
        ),
        assign(
            place(6, reference),
            Rvalue::Use(Operand::Copy(place(5, reference))),
        ),
    ];
    let mut parent = 6;
    for ordinal in 0..depth {
        let child = 8 + ordinal as u32;
        statements.push(assign(
            place(child, reference),
            Rvalue::Borrow {
                kind: BorrowKind::Shared,
                place: Place::new(
                    SemanticLocalIdV1::from_index(parent),
                    vec![SemanticProjectionV1::new(Projection::Dereference, witness).unwrap()],
                    witness,
                )
                .unwrap(),
            },
        ));
        parent = child;
    }
    let blocks = vec![
        block(0, vec![], call(issue, vec![], 4, witness, 1)),
        block(
            1,
            statements,
            call(
                issue + 1,
                vec![Operand::Copy(place(parent, reference))],
                7,
                raw,
                2,
            ),
        ),
        block(
            2,
            vec![assign(
                place(0, helper.abi().source_output_type()),
                Rvalue::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: Operand::Copy(place(7, raw)),
                },
            )],
            Terminator::Return,
        ),
    ];
    *helper = Function::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        source,
        helper.abi().clone(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
}

fn run(depth: usize, work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_callable_transform(
        work,
        storage,
        |types, functions, callables| fixture(types, functions, callables, depth),
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let mut program = super::super::super::source_function::SourceByteProgram::derive(
                    plan, slots, out,
                )?;
                program.emit(out)?;
                for root in 0..2 {
                    for instance in 1..=2 {
                        let name =
                            format!("spec fn invocation_source_byte_event_{root}_{instance}_v36");
                        let rest =
                            &out.text[out.text.find(&name).expect("original helper events")..];
                        let end = rest[1..].find("spec fn ").map_or(rest.len(), |at| at + 1);
                        let table = &rest[..end];
                        assert_eq!(
                            table
                                .matches("parent: Some(InvocationSourceWitnessParentV43")
                                .count(),
                            depth
                        );
                        assert_eq!(table.matches("parent: None").count(), 1);
                    }
                }
                Ok(())
            })
        },
    )
}

#[test]
fn original_source_witness_reborrow_program_preserves_parent_site_on_both_roots() {
    for depth in [1, 2, 4] {
        run(depth, LIMIT, LIMIT).0.unwrap();
    }
}

#[test]
fn expanded_source_scalar_relations_preserve_genuine_witness_reborrows() {
    use super::super::super::{
        paired::ExpandedScalarBindingsV196, slots::tests::with_tile_slots,
        tile_target::TileTargetV176,
    };
    use fe2o3_kernel_ir::{ExecutionTileLayoutV1 as Layout, FormalIndexWidth};

    for layout in [Layout::Blocked, Layout::Striped] {
        super::super::super::super::invocations::tests::run_callable_transform(
            LIMIT,
            LIMIT,
            |types, functions, callables| fixture(types, functions, callables, 2),
            |plan, out| {
                with_tile_slots(plan, layout, out, |slots, out| {
                    let target = TileTargetV176::derive(slots, out)?;
                    let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
                    for root in 0..2 {
                        for instance in 1..=2 {
                            let row = plan.instance(root, instance, out)?;
                            for (statement, local) in [(0, 5), (1, 6), (2, 8), (3, 9)] {
                                let value = original_value(
                                    slots, row.function, 1, Some(statement),
                                    OperandRole::Destination, local, true, out,
                                )?;
                                let endpoint = slots.correspondence(out)?
                                    .ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
                                let reference = endpoint.reference(out.budget)?.unwrap();
                                assert_eq!(reference.carrier(out.budget)?, Carrier::StableScalar);
                                let ty = reference.origin_type(out.budget)?.index();
                                let origin_row = plan.instance(root, reference.origin_instance(out.budget)?, out)?;
                                let origin = origin_row.locals.start + reference.origin_local(out.budget)?.index() as usize;
                                let generation = reference.origin_generation(out.budget)?;
                                let (borrow_instance, borrow_block, borrow_statement) = reference.borrow_site(out.budget)?;
                                let borrow_block = borrow_block.index();
                                let borrow_statement = borrow_statement.unwrap();
                                let start = out.text.len();
                                pairs.emit_source_conjunct(
                                    plan, root, instance, value, FormalIndexWidth::Bits64, out,
                                )?;
                                let text = &out.text[start..];
                                let local = row.locals.start + local as usize;
                                assert!(text.contains(&format!("invocation_source_reference_current_v38(source, {local}, {ty})")));
                                assert!(text.contains(&format!("reference.origin == {origin} && reference.origin_generation == {generation} && reference.borrow_instance == {borrow_instance} && reference.borrow_block == {borrow_block} && reference.borrow_statement == {borrow_statement}")));
                                // Both sibling invocations have dynamic source depth one.
                                assert!(text.contains(&format!("source.machine.frames.active[1].owner == {}", row.function.index())));
                                assert!(!text.contains("source.machine.frames.active[2]"));
                            }
                        }
                    }
                    Ok(())
                })
            },
        ).0.unwrap();
    }
}

#[test]
fn original_source_witness_reborrow_program_has_exact_resource_boundaries() {
    let generous = run(2, LIMIT, LIMIT);
    generous.0.unwrap();
    let exact = run(2, generous.1, generous.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (generous.1, generous.2, generous.3)
    );
    for (work, storage, is_work) in [
        (generous.1 - 1, generous.3, true),
        (generous.1, generous.3 - 1, false),
    ] {
        let error = run(2, work, storage).0.unwrap_err();
        let mut chain: &(dyn std::error::Error + 'static) = &error;
        let resource = loop {
            if let Some(resource) = chain.downcast_ref::<Resource>() {
                break resource;
            }
            chain = chain
                .source()
                .unwrap_or_else(|| panic!("missing resource: {error:?}"));
        };
        match resource {
            Resource::Work(limit) if is_work => {
                assert_eq!(limit.limit(), work);
                assert!(limit.actual() > work);
            }
            Resource::Storage(limit) if !is_work => {
                assert_eq!(limit.limit(), storage);
                assert!(limit.actual() > storage);
            }
            other => panic!("wrong resource: {other:?}"),
        }
    }
}

fn emission(plan: Borrow, work: usize, storage: usize) -> (Result<String>, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    let result = (|| {
        budget.reserve_storage(SOURCE_LIMIT + headers())?;
        let mut out = Writer::new(&mut budget)?;
        plan.emit(&mut out)?;
        out.finish()
    })();
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn original_witness_reborrow_emission_has_independent_text_and_resource_oracle() {
    for parent in [None, Some((8, 1, 2, 3))] {
        let plan = Borrow {
            destination: 9,
            origin: 4,
            source_type: 3,
            generation: 0,
            instance: 1,
            block: 2,
            statement: 4,
            parent,
        };
        let expected_parent = if parent.is_some() {
            "Some(InvocationSourceWitnessParentV43 { local: 8, instance: 1, block: 2, statement: 3 })"
        } else {
            "None"
        };
        let expected = format!(
            "InvocationSourceByteEventV36::WitnessBorrow {{ destination: 9, origin: 4, source_type: 3, generation: 0, instance: 1, block: 2, statement: 4, parent: {expected_parent} }}"
        );
        let work = 1 + expected.len();
        let storage = SOURCE_LIMIT + headers();
        let actual = emission(plan, work, storage);
        assert_eq!(actual.0.unwrap(), expected);
        assert_eq!((actual.1, actual.2), (work, storage));
        assert!(
            matches!(emission(plan, work - 1, storage).0, Err(Error::Resource(Resource::Work(limit))) if limit.limit() == work - 1 && limit.actual() == work)
        );
        assert!(
            matches!(emission(plan, work, storage - 1).0, Err(Error::Resource(Resource::Storage(limit))) if limit.limit() == storage - 1 && limit.actual() == storage)
        );
    }
}

#[test]
fn original_witness_reborrow_runtime_preserves_origin_version_frame_and_exact_parent_site() {
    let model = include_str!("original_semantic_mir_source_logical_locals_v38.vrs");
    let start = model
        .find("spec fn invocation_source_reborrow_witness_v43(")
        .unwrap();
    let end = model[start..]
        .find("spec fn invocation_source_read_witness_v38(")
        .unwrap()
        + start;
    let body = &model[start..end];
    for guard in [
        "invocation_source_reference_current_v38(source, parent.local, source_type)",
        "origin != origin",
        "origin_generation != origin_generation",
        "borrow_instance != parent.instance",
        "borrow_block != parent.block",
        "borrow_statement != parent.statement",
        "destination == origin",
    ] {
        assert!(body.contains(guard), "{guard}");
    }
    assert!(body.contains("..source.logical.references[parent.local]"));
    assert!(!body.contains("frames.active.last()"));
    assert!(
        super::super::SOURCE_BYTES_V36
            .contains("statement, parent } => invocation_source_reborrow_witness_v43(")
    );
}

#[test]
fn original_witness_reborrow_headers_cover_parent_and_both_borrowed_endpoints() {
    type Fields = (
        usize,
        usize,
        u32,
        u32,
        usize,
        usize,
        usize,
        Option<(usize, usize, usize, usize)>,
    );
    assert_eq!(size_of::<Borrow>(), size_of::<Fields>());
    let expected = size_of::<Fields>()
        + size_of::<Result<Option<Fields>>>()
        + size_of::<SsaValueV1>()
        + size_of::<Result<SsaValueV1>>()
        + 2 * size_of::<[u32; 2]>()
        + size_of::<Option<SsaValueV1>>()
        + size_of::<Site>()
        + size_of::<OperandRole>()
        + size_of::<EventRole>()
        + size_of::<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>()
        + size_of::<Result<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>>()
        + 30 * size_of::<usize>()
        + 20 * size_of::<&()>()
        + 2 * size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + 2 * size_of::<fe2o3_lower_mir_kernel::ProductionSourceReferenceEndpointV38<'_, '_>>()
        + size_of::<
            Result<Option<fe2o3_lower_mir_kernel::ProductionSourceReferenceEndpointV38<'_, '_>>>,
        >()
        + size_of::<Option<(usize, usize, usize, usize)>>();
    assert_eq!(headers(), expected);
}

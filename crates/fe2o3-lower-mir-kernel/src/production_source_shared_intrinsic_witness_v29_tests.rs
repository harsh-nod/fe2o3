use super::*;

const SHARED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const BOOL: SemanticTypeIdV1 = REFERENCE;
const OTHER_WITNESS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const LIMIT: usize = 20_000_000;

#[derive(Clone, Copy, Debug)]
enum Witness {
    Grid,
    Block,
    Tile,
    Stripe,
}

impl Witness {
    fn space(self) -> SemanticDisjointIndexSpaceV1 {
        match self {
            Self::Grid => SemanticDisjointIndexSpaceV1::GridExclusive,
            Self::Block => SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                lanes_per_block: 64,
                elements_per_lane: 1,
            },
            Self::Tile => SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                lanes_per_tile: 64,
                tile_rows: 8,
                tile_columns: 8,
                elements_per_lane: 1,
            },
            Self::Stripe => SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                lanes_per_row: 64,
                elements_per_lane: 1,
            },
        }
    }

    fn operation(self, write: bool) -> SemanticCompilerIntrinsicOperationV1 {
        if write {
            let kind = match self {
                Self::Grid => SemanticWriteOnlyDisjointWriteKindV1::GridExclusive,
                Self::Block => SemanticWriteOnlyDisjointWriteKindV1::Block {
                    lanes_per_block: 64,
                    elements_per_lane: 1,
                },
                Self::Tile => SemanticWriteOnlyDisjointWriteKindV1::Tiled2d {
                    lanes_per_tile: 64,
                    tile_rows: 8,
                    tile_columns: 8,
                    elements_per_lane: 1,
                },
                Self::Stripe => SemanticWriteOnlyDisjointWriteKindV1::RowStriped2d {
                    lanes_per_row: 64,
                    elements_per_lane: 1,
                },
            };
            return SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                disjoint_slice: CARRIER,
                witness: WITNESS,
                element: U32,
                raw_index: INDEX,
                index_space: self.space(),
                kind,
            };
        }
        match self {
            Self::Grid => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                disjoint_slice: CARRIER,
                grid_leader: WITNESS,
                element: U32,
                raw_index: INDEX,
            },
            Self::Block => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                disjoint_slice: CARRIER,
                block_witness: WITNESS,
                element: U32,
                raw_index: INDEX,
                index_space: self.space(),
                lanes_per_block: 64,
                elements_per_lane: 1,
            },
            Self::Tile => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                disjoint_slice: CARRIER,
                tile_witness: WITNESS,
                element: U32,
                raw_index: INDEX,
                index_space: self.space(),
                lanes_per_tile: 64,
                tile_rows: 8,
                tile_columns: 8,
                elements_per_lane: 1,
            },
            Self::Stripe => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
                disjoint_slice: CARRIER,
                stripe_witness: WITNESS,
                element: U32,
                raw_index: INDEX,
                index_space: self.space(),
                lanes_per_row: 64,
                elements_per_lane: 1,
            },
        }
    }
}

fn witness_abi(
    tag: u8,
    kernel: bool,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    witness: Witness,
    other_witness: SemanticTypeIdV1,
) -> SemanticFunctionAbiV1 {
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let borrowed = |shared: bool| {
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                true,
                shared,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            if shared && !matches!(witness, Witness::Block) {
                8
            } else {
                16
            },
            Some(8),
        )
        .unwrap()
    };
    let arguments = inputs
        .iter()
        .map(|&ty| {
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                ty,
                if ty == CARRIER
                    || ((ty == WITNESS || ty == other_witness) && matches!(witness, Witness::Block))
                {
                    SemanticAbiPassModeV1::Pair {
                        first: plain,
                        second: plain,
                    }
                } else if ty == SHARED || ty == BORROW {
                    SemanticAbiPassModeV1::Direct(borrowed(ty == SHARED))
                } else {
                    SemanticAbiPassModeV1::Direct(plain)
                },
            ))
        })
        .collect();
    let output_mode = if output == UNIT {
        SemanticAbiPassModeV1::Ignore
    } else if output == OPTIONAL {
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                Some(4),
            )
            .unwrap(),
        )
    } else if output == BOOL {
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::ZeroExtend,
                0,
                None,
            )
            .unwrap(),
        )
    } else {
        SemanticAbiPassModeV1::Direct(plain)
    };
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if kernel {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if kernel {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        inputs.len() as u32,
        arguments,
        SemanticAbiValueV1::new(output, output_mode),
    )
    .unwrap()
    .with_source_argument_ownership(
        inputs
            .iter()
            .map(|&ty| {
                if ty == CARRIER {
                    SemanticSourceArgumentOwnershipV1::ExclusiveOwner
                } else if ty == BORROW {
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow
                } else if ty == SHARED {
                    SemanticSourceArgumentOwnershipV1::SharedBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::ByValue
                }
            })
            .collect(),
    )
    .unwrap()
}

// An admitted original MIR/SSA component, not authority to manufacture a trusted
// provider capability in Rust. The actual borrowed-source driver covers that join.
fn witness_owner(witness: Witness, write: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = owner_with_shape_uncaptured(1, 0);
    let mut types = base.source_semantic().types().to_vec();
    if matches!(witness, Witness::Block) {
        types[WITNESS.index() as usize] = declaration(
            8,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(integer(64), integer(64)),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![INDEX, INDEX, UNIT]).unwrap(),
            ),
        );
    }
    assert_eq!(types.len(), SHARED.index() as usize);
    types.push(
        declaration(
            10,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    WITNESS,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        if matches!(witness, Witness::Block) {
                            16
                        } else {
                            8
                        },
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    // Write-only calls have neither the reference nor Option return subtree.
    // Reuse those dense slots rather than retaining unreachable declarations.
    if write {
        types[BOOL.index() as usize] = declaration(
            11,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(1),
                1,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 8, 1),
                    SemanticScalarValidityRangeV1::new(0, 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        );
    }
    let other_witness = if write { OPTIONAL } else { OTHER_WITNESS };
    let other = declaration(
        12,
        types[WITNESS.index() as usize].layout().clone(),
        types[WITNESS.index() as usize].shape().clone(),
    );
    if write {
        types[other_witness.index() as usize] = other;
    } else {
        assert_eq!(types.len(), other_witness.index() as usize);
        types.push(other);
    }
    let result = if write { BOOL } else { OPTIONAL };
    // The hostile same-shape identity has a genuine independent source ABI
    // argument, never an unreachable type appended just for a plan mutation.
    let local_types = [
        UNIT,
        CARRIER,
        WITNESS,
        BORROW,
        SHARED,
        result,
        other_witness,
    ];
    let locals = local_types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([40 + index as u8; 32]),
                ty,
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    1 => SemanticLocalRoleV1::Argument(0),
                    2 => SemanticLocalRoleV1::Argument(1),
                    6 => SemanticLocalRoleV1::Argument(2),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                provenance(),
            )
        })
        .collect();
    let mut inputs = vec![BORROW, SHARED];
    inputs.extend(std::iter::repeat_n(
        INDEX,
        if matches!(witness, Witness::Tile | Witness::Stripe) {
            4
        } else {
            1
        },
    ));
    if write {
        inputs.push(U32);
    }
    let mut arguments = vec![
        SemanticOperandV1::Move(place(3, BORROW)),
        SemanticOperandV1::Copy(place(4, SHARED)),
    ];
    arguments.extend(inputs[2..].iter().map(|&ty| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(0, if ty == INDEX { 8 } else { 4 }).unwrap(),
            ),
        ))
    }));
    let blocks = vec![
        block(
            0,
            vec![
                assign(
                    3,
                    BORROW,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: place(1, CARRIER),
                    },
                ),
                assign(
                    4,
                    SHARED,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(2, WITNESS),
                    },
                ),
            ],
            call(1, arguments, 5, result, 1),
        ),
        block(
            1,
            vec![assign(
                0,
                UNIT,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ];
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([20; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([20; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([20; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([20; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([20; 32]),
        provenance(),
        witness_abi(
            20,
            true,
            &[CARRIER, WITNESS, other_witness],
            UNIT,
            witness,
            other_witness,
        ),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(
        base.source_semantic().functions()[0]
            .kernel_entry()
            .unwrap()
            .clone(),
    );
    let source = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            intrinsic(
                21,
                witness_abi(21, false, &inputs, result, witness, other_witness),
                witness.operation(write),
            ),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn with_witness_plan(witness: Witness, write: bool) {
    let mut owner = witness_owner(witness, write);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let mut completed = false;
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = with_source_reference_plan_v29(instances, budget, |plan, _| {
                assert_eq!(plan.loans.len(), 2);
                let carrier = &plan.loans[0];
                let capability = &plan.loans[1];
                assert_eq!(carrier.kind, SemanticBorrowKindV1::Mutable);
                assert_eq!(carrier.effects.referent_reads, 1);
                assert_eq!(carrier.effects.payload_reads, usize::from(!write));
                assert_eq!(carrier.effects.payload_writes, 1);
                assert_eq!(capability.kind, SemanticBorrowKindV1::Shared);
                assert_eq!(capability.source_type, SHARED);
                assert_eq!(plan.origins[capability.origin].ty, WITNESS);
                assert_eq!(
                    capability.effects,
                    SourceReferenceEffectsV29 {
                        referent_reads: 1,
                        ..Default::default()
                    }
                );
                assert_eq!(
                    capability.representation,
                    SourceReferenceRepresentationV29::StableReferent
                );
                assert!(matches!(
                    carrier.representation,
                    SourceReferenceRepresentationV29::ExistingAllocationBinding(_)
                ));
                completed = true;
                Ok(())
            });
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap()
    .unwrap();
    assert!(completed);
}

#[test]
fn shared_intrinsic_capabilities_read_only_their_original_referents() {
    for witness in [
        Witness::Grid,
        Witness::Block,
        Witness::Tile,
        Witness::Stripe,
    ] {
        with_witness_plan(witness, false);
    }
}

#[test]
fn shared_write_only_capabilities_preserve_descriptor_write_only_effects() {
    for witness in [
        Witness::Grid,
        Witness::Block,
        Witness::Tile,
        Witness::Stripe,
    ] {
        with_witness_plan(witness, true);
    }
}

fn with_witness_builder(
    consume: impl FnOnce(
        &mut SourceReferenceBuilderV29<'_, '_, '_>,
        &SemanticCallableDeclV1,
        &SemanticDirectCallV1,
        &mut Vec<usize>,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    let mut owner = witness_owner(Witness::Grid, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let mut completed = false;
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            builder.function(instances.root(), None, budget).unwrap();
            // Restore a genuine post-call frame. It retains both original argument
            // objects; the loan nodes themselves come from the original borrow sites.
            let entry = builder
                .plan
                .blocks
                .iter()
                .find(|row| row.block.index() == 1)
                .unwrap()
                .entry;
            builder.frames[instances.root().index()] = Some(entry);
            let semantic = instances.owner().source_semantic();
            let callable = &semantic.callables()[1];
            let SemanticTerminatorKindV1::Call(call) =
                semantic.functions()[0].blocks()[0].terminator().kind()
            else {
                panic!("fixture lost its original intrinsic call");
            };
            let carrier = builder
                .plan
                .nodes
                .iter()
                .position(|row| row.kind == SourceReferenceNodeKindV29::Loan(0))
                .unwrap();
            let witness = builder
                .plan
                .nodes
                .iter()
                .position(|row| row.kind == SourceReferenceNodeKindV29::Loan(1))
                .unwrap();
            let index = builder.plain(INDEX, budget).unwrap();
            let mut arguments = vec![carrier, witness, index];
            consume(&mut builder, callable, call, &mut arguments, budget);
            completed = true;
            drop(arguments);
            drop(builder);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
    assert!(completed);
}

fn exact_reason(error: ProductionSemanticKirErrorV1, expected: &'static str) {
    assert_eq!(
        error.to_string(),
        source_reference_error_v29(expected).to_string()
    );
}

#[test]
fn shared_intrinsic_witness_requires_exact_original_loan_node_type_and_role() {
    for mode in 0..8 {
        with_witness_builder(|builder, callable, call, arguments, budget| {
            let witness = arguments[1];
            let origin = builder.plan.loans[1].origin;
            let before = builder.plan.loans[1].effects;
            match mode {
                0 => builder.plan.loans[1].source_type = BORROW,
                1 => builder.plan.loans[1].kind = SemanticBorrowKindV1::Mutable,
                2 => builder.plan.origins[origin].ty = INDEX,
                3 => builder.plan.nodes[witness].ty = BORROW,
                4 => {
                    // A same-typed aggregate wrapper cannot smuggle an inner loan
                    // into the intrinsic's single shared-reference operand.
                    let first = builder.plan.children.len();
                    emission_push_v1(&mut builder.plan.children, witness, budget).unwrap();
                    arguments[1] = builder
                        .node(
                            SHARED,
                            SourceReferenceNodeKindV29::Aggregate { first, count: 1 },
                            budget,
                        )
                        .unwrap();
                }
                5 => arguments.swap(1, 2),
                6 => arguments.swap(0, 1),
                7 => {
                    let types = builder.plan.instances.owner().source_semantic().types();
                    assert_eq!(
                        types[OTHER_WITNESS.index() as usize].shape(),
                        types[WITNESS.index() as usize].shape()
                    );
                    assert_ne!(OTHER_WITNESS, WITNESS);
                    builder.plan.origins[origin].ty = OTHER_WITNESS;
                }
                _ => unreachable!(),
            }
            let error = builder
                .intrinsic(callable, arguments, call, budget)
                .unwrap_err();
            exact_reason(
                error,
                if matches!(mode, 5 | 6) {
                    "source reference allocation-view operand differs"
                } else {
                    "source reference shared intrinsic witness differs"
                },
            );
            assert_eq!(builder.plan.loans[1].effects, before);
        });
    }
}

#[test]
fn shared_intrinsic_witness_keeps_dead_and_replaced_original_loan_refusals() {
    for mode in 0..2 {
        with_witness_builder(|builder, callable, call, arguments, budget| {
            let origin = &builder.plan.origins[builder.plan.loans[1].origin];
            let (instance, local) = (origin.instance, origin.local);
            let mut state = builder.local(instance, local).unwrap();
            if mode == 0 {
                state.node = None;
            } else {
                state.generation += 1;
            }
            builder.set_local(instance, local, state).unwrap();
            let before = builder.plan.loans[1].effects;
            exact_reason(
                builder
                    .intrinsic(callable, arguments, call, budget)
                    .unwrap_err(),
                "source reference referent is dead or replaced",
            );
            assert_eq!(builder.plan.loans[1].effects, before);
        });
    }
}

#[test]
fn shared_intrinsic_witness_is_not_a_generic_intrinsic_or_thread_operand_permission() {
    for thread in [false, true] {
        with_witness_builder(|builder, callable, call, arguments, budget| {
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation_identity,
                ..
            } = callable
            else {
                unreachable!()
            };
            // Deliberately inconsistent metadata is a hostile private-boundary
            // control, never an admitted source fixture or production authority.
            let changed = SemanticCallableDeclV1::CompilerIntrinsic {
                binding: binding.clone(),
                operation_identity: *operation_identity,
                operation: if thread {
                    SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                        disjoint_slice: CARRIER,
                        witness: WITNESS,
                        element: U32,
                        raw_index: INDEX,
                        index_space: SemanticDisjointIndexSpaceV1::GridExclusive,
                        kind: SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: false },
                    }
                } else {
                    SemanticCompilerIntrinsicOperationV1::ColdPath
                },
            };
            let before = builder.plan.loans[1].effects;
            exact_reason(
                builder
                    .intrinsic(&changed, arguments, call, budget)
                    .unwrap_err(),
                if thread {
                    "source reference allocation-view operand differs"
                } else {
                    "source reference intrinsic effect is not represented"
                },
            );
            assert_eq!(builder.plan.loans[1].effects, before);
        });
    }
}

fn measured_witness_plan(
    owner: &ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let mut completed = false;
    let result = production_call_instances_v1::with_production_call_instances_v1(
        owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = with_source_reference_plan_v29(instances, budget, |plan, _| {
                assert_eq!(
                    plan.loans[1].effects,
                    SourceReferenceEffectsV29 {
                        referent_reads: 1,
                        ..Default::default()
                    }
                );
                completed = true;
                Ok(())
            });
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    );
    let result = match result {
        Ok(result) => result,
        Err(production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error)) => {
            Err(error.into())
        }
        Err(error) => panic!("unexpected source call-plan refusal: {error}"),
    };
    assert_eq!(budget.storage(), 0);
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn shared_intrinsic_witness_plan_exact_and_one_short_resources_restore_the_outer_floor() {
    let mut owner = witness_owner(Witness::Grid, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    let (result, work, peak, completed) = measured_witness_plan(&owner, LIMIT, LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, used, retained, completed) = measured_witness_plan(&owner, work, peak);
    result.unwrap();
    assert!(completed);
    assert_eq!((used, retained), (work, peak));
    for (work_limit, storage_limit, is_work) in [(work - 1, peak, true), (work, peak - 1, false)] {
        let (result, _, _, completed) = measured_witness_plan(&owner, work_limit, storage_limit);
        assert!(!completed);
        match result {
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(error),
            )) if is_work => assert_eq!(error.limit(), work_limit),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error),
            )) if !is_work => assert_eq!(error.limit(), storage_limit),
            result => panic!("wrong witness resource refusal: {result:?}"),
        }
    }
}

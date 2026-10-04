//! Real once-constructed graph owners; no source request or conditional proof.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CheckedBinaryOperator,
    ComparePredicate, Constant, Function, InertCanonicalKirTransitionGraphIdentityV1 as Identity,
    InertCanonicalKirTransitionReceiptV1 as Wire, MemoryAccess, Module, Operation,
    OperationKind as Kind, ScalarType, Signature, Terminator, Type, UnaryOp, ValueDef, ValueId,
};
use fe2o3_kernel_opt::*;
use fe2o3_pliron::{
    OwnedCommutativeBitwiseContinuationV1, prepare_owned_commutative_bitwise_continuation_v1,
};

#[path = "production_conditional_checked_output_fixture_v1_tests.rs"]
#[allow(
    dead_code,
    reason = "reuse the existing component module and owning prefix constructors"
)]
mod base;
pub(super) const FLOOR: usize = base::FLOOR;
pub(super) const STORAGE: usize = base::STORAGE;
pub(super) const WORK: usize = base::WORK;

pub(super) struct Complete {
    pub(super) n: Graph,
    pub(super) b: Graph,
    pub(super) checked: Prefix,
    pub(super) j: OwnedRedundantStoreContinuationV1,
    pub(super) k: OwnedCommutativeBitwiseContinuationV1,
    pub(super) p: OwnedPrivateCellPromotionContinuationV1,
    pub(super) h: OwnedLoopPreheadersContinuationV1,
    pub(super) l: OwnedLicmContinuationV1,
    pub(super) r: OwnedInductionRefinementContinuationV1,
    pub(super) f: OwnedCrossBlockForwardingV1,
    p4: InertCanonicalPolicy4ExecutionReceiptV1,
    wire: Wire,
    transcript: Vec<u8>,
}
impl Complete {
    pub(super) fn inputs(&self) -> Inputs<'_> {
        let p5 = self.checked.intermediate_policy5();
        let p4 = p5.intermediate_policy4();
        Inputs {
            prefix: CanonicalPolicy8SemanticInputsV1 {
                prefix: CanonicalPolicy7SemanticInputsV1 {
                    prefix: CanonicalPolicy6SemanticInputsV1 {
                        prefix: CanonicalPolicy5SemanticInputsV1 {
                            input: &self.b,
                            intermediate: p4.intermediate_policy3().owner(),
                            stored: p4.owner(),
                            output: p5.owner(),
                            policy4_wire: self.p4.canonical_bytes(),
                            policy5_record: p5.execution().canonical_bytes(),
                            load_rows: p5.load_forwarding_rows(),
                        },
                        output: self.checked.owner(),
                        continuation: CanonicalPolicy6ContinuationClaimsV1 {
                            composition_record: self.checked.execution().canonical_bytes(),
                            integer_record: self
                                .checked
                                .continuation()
                                .execution()
                                .canonical_bytes(),
                            transition_wire: self.wire.canonical_bytes(),
                        },
                    },
                    output: self.j.output(),
                    continuation: CanonicalPolicy7ContinuationClaimsV1 {
                        execution_record: &self.transcript,
                        deletion_rows: self.j.rows(),
                        retained_operations: self.j.retained_operations(),
                    },
                },
                output: self.k.output(),
                continuation: CanonicalPolicy8ContinuationClaimsV1 {
                    pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
                    input: Identity::from_verified(self.j.output().canonical().identity()),
                    output: Identity::from_verified(self.k.output().canonical().identity()),
                    occurrences: self.k.occurrences().candidate(),
                },
            },
            promoted: self.p.output(),
            selected_allocations: self.p.selected_allocations(),
            promotion_origins: self.p.origins(),
            preheaders: self.h.output(),
            preheader_rows: self.h.preheaders(),
            licm: self.l.output(),
            licm_origins: self.l.origins(),
            refined: self.r.output(),
            refinement_origins: self.r.origins(),
            output: self.f.output(),
            forwarding_origins: self.f.origins(),
            limits: Limits {
                refinement: self.r.limits(),
                forwarding: self.f.limits(),
            },
        }
    }
}

// F2P7EX1 is an INERT transcript claim, using the same closed layout as the
// kernel-opt component tests. It is independently checked, never an execution
// witness, source request, reference receipt, signature, or conditional proof.
fn transcript(
    checked: &Prefix,
    j: &OwnedRedundantStoreContinuationV1,
    budget: &mut Budget<'_>,
) -> Vec<u8> {
    let extent = 384 + (j.rows().len() + j.retained_operations().len()) * 24;
    budget.charge_work(extent).unwrap();
    budget
        .reserve_storage(extent + size_of::<Vec<u8>>())
        .unwrap();
    let mut bytes = Vec::with_capacity(extent);
    budget.reserve_storage(bytes.capacity() - extent).unwrap();
    bytes.resize(384, 0);
    bytes[..16].copy_from_slice(b"F2P7EX1\0\x01\0\x07\0\0\x01\0\0");
    bytes[16..272].copy_from_slice(checked.execution().canonical_bytes());
    for (offset, graph) in [(272, checked.owner()), (312, j.output())] {
        let identity = graph.canonical().identity();
        bytes[offset..offset + 32].copy_from_slice(identity.digest());
        bytes[offset + 32..offset + 40].copy_from_slice(&identity.canonical_length().to_le_bytes());
    }
    for (offset, count) in [
        (352, j.rows().len()),
        (360, j.retained_operations().len()),
        (368, 1),
        (376, extent),
    ] {
        bytes[offset..offset + 8].copy_from_slice(&(count as u64).to_le_bytes());
    }
    let mut coordinate = |site: Site| {
        for value in [site.block.function.0, site.block.block, site.operation] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    };
    for row in j.rows() {
        coordinate(row.anchor);
        coordinate(row.removed);
    }
    for row in j.retained_operations() {
        coordinate(row.input);
        coordinate(row.output);
    }
    assert_eq!(bytes.len(), extent);
    bytes
}

pub(super) fn with_complete<T>(
    profile: Profile,
    changed: bool,
    action: impl FnOnce(&Complete, &mut Budget<'_>) -> T,
) -> T {
    with_module(profile, module(changed), action)
}

pub(super) fn with_same_parameter_reads<T>(
    profile: Profile,
    action: impl FnOnce(&Complete, &mut Budget<'_>) -> T,
) -> T {
    with_module(
        profile,
        base::module_with_input_arguments(false, [1, 1]),
        action,
    )
}

pub(super) fn with_helper_motion<T>(
    profile: Profile,
    action: impl FnOnce(&Complete, &mut Budget<'_>) -> T,
) -> T {
    let mut module = module(false);
    module.functions.push(helper_motion());
    with_module(profile, module, action)
}

fn with_module<T>(
    profile: Profile,
    module: Module,
    action: impl FnOnce(&Complete, &mut Budget<'_>) -> T,
) -> T {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let n = base::graph(&module, &mut budget);
    drop(module);
    let binding = dialect_amdgcn::bind_production_target_v1(n.module(), profile).unwrap();
    let b = base::graph(binding.module(), &mut budget);
    drop(binding);
    let checked = base::prefix(&b, &mut budget);
    let p4 = encode_checked_canonical_policy4_execution_receipt_v1(
        &b,
        checked.intermediate_policy5().intermediate_policy4(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(p4.storage().retained_storage())
        .unwrap();
    let (wire, storage) = Wire::from_candidate_with_budget(
        checked
            .intermediate_policy5()
            .owner()
            .canonical()
            .identity(),
        checked.owner().canonical().identity(),
        checked.continuation().occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let j = prepare_owned_redundant_store_continuation_v1(checked.owner(), &mut budget).unwrap();
    budget.reserve_storage(j.retained_storage()).unwrap();
    let transcript = transcript(&checked, &j, &mut budget);
    let k = prepare_owned_commutative_bitwise_continuation_v1(j.output(), &mut budget).unwrap();
    budget.reserve_storage(k.retained_storage()).unwrap();
    let p = prepare_owned_private_cell_promotion_v1(k.output(), &mut budget).unwrap();
    budget.reserve_storage(p.retained_storage()).unwrap();
    let h = prepare_owned_loop_preheaders_v1(p.output(), &mut budget).unwrap();
    budget.reserve_storage(h.retained_storage()).unwrap();
    let l = prepare_owned_licm_v1(h.output(), &mut budget).unwrap();
    budget.reserve_storage(l.retained_storage()).unwrap();
    let r =
        prepare_owned_induction_refinement_v1(l.output(), Default::default(), &mut budget).unwrap();
    budget.reserve_storage(r.retained_storage()).unwrap();
    let f = prepare_owned_cross_block_forwarding_v1(r.output(), Default::default(), &mut budget)
        .unwrap();
    budget.reserve_storage(f.retained_storage()).unwrap();
    let complete = Complete {
        n,
        b,
        checked,
        j,
        k,
        p,
        h,
        l,
        r,
        f,
        p4,
        wire,
        transcript,
    };
    let floor = budget.storage();
    let account = budget.work_ledger_identity_v1();
    let result = action(&complete, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == account);
    drop(complete);
    budget.release_storage(floor - FLOOR).unwrap();
    result
}

pub(super) fn graph(module: &Module, budget: &mut Budget<'_>) -> Graph {
    base::graph(module, budget)
}

pub(super) fn module(changed: bool) -> Module {
    let mut module = base::module(false);
    if changed {
        module.functions[0]
            .signature
            .parameters
            .push(Type::Scalar(ScalarType::U32));
        let body = module.functions[0].body.as_mut().unwrap();
        body.parameters.push(ValueId(3));
        let block = &mut body.blocks[1];
        block.operations.truncate(block.operations.len() - 3);
        // Both commuted definitions are live before K. K shares the second use
        // and shifts the SECOND READ and output store, not just an inert row.
        let at = block
            .operations
            .iter()
            .position(|op| op.results.iter().any(|r| r.id == ValueId(32)))
            .unwrap()
            + 1;
        block.operations.splice(
            at..at,
            [(52, 32, 3), (53, 3, 32)].map(|(id, lhs, rhs)| {
                Operation::effect_free(
                    ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
                    Kind::Binary {
                        op: BinaryOp::BitOr,
                        lhs: ValueId(lhs),
                        rhs: ValueId(rhs),
                    },
                )
            }),
        );
        for (id, lhs, rhs) in [(54, 52, 53), (55, 54, 42)] {
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
                Kind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(lhs),
                    rhs: ValueId(rhs),
                },
            ));
        }
        block.operations.push(Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(21),
                value: ValueId(55),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        module.functions.push(loop_copy());
        module.functions.push(private_cell());
    }
    module
}

fn block(
    id: u32,
    parameters: Vec<ValueDef>,
    operations: Vec<Operation>,
    terminator: Terminator,
) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.parameters = parameters;
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn jump(target: u32, values: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: values.iter().copied().map(ValueId).collect(),
    }
}

// This uncalled helper is not conditional coverage: it exercises actual R/F
// rewrites in the SAME complete module. Private-load conditional coverage below
// remains refused; the helper is not relabeled as an admitted conditional body.
fn loop_copy() -> Function {
    let ty = Type::Scalar(ScalarType::U64);
    let private = MemoryAccess::new(AddressSpace::Private, 8);
    Function::internal_helper(
        "loop_copy",
        Signature::new(
            vec![
                ty.clone(),
                ty.clone(),
                Type::pointer(ty.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(7), ValueId(8)],
        vec![
            block(
                10,
                vec![],
                vec![
                    Operation::effect_free(
                        ValueDef::new(ValueId(2), ty.clone()),
                        Kind::Constant(Constant::U64(1)),
                    ),
                    Operation::effect_free(
                        ValueDef::new(
                            ValueId(100),
                            Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                        ),
                        Kind::Alloca {
                            element: ty.clone(),
                            count: None,
                            address_space: AddressSpace::Private,
                            alignment: 8,
                        },
                    ),
                ],
                jump(20, &[0]),
            ),
            block(
                20,
                vec![ValueDef::new(ValueId(3), ty.clone())],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(4), Type::BOOL),
                    Kind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(3),
                        rhs: ValueId(1),
                    },
                )],
                Terminator::ConditionalBranch {
                    condition: ValueId(4),
                    then_target: BlockId(30),
                    then_arguments: vec![],
                    else_target: BlockId(50),
                    else_arguments: vec![],
                },
            ),
            block(
                30,
                vec![],
                vec![Operation::new(
                    vec![],
                    Kind::Store {
                        pointer: ValueId(100),
                        value: ValueId(3),
                        access: private,
                    },
                )],
                Terminator::ConditionalBranch {
                    condition: ValueId(8),
                    then_target: BlockId(40),
                    then_arguments: vec![],
                    else_target: BlockId(45),
                    else_arguments: vec![],
                },
            ),
            block(
                40,
                vec![],
                vec![
                    Operation::effect_free(
                        ValueDef::new(ValueId(101), ty.clone()),
                        Kind::Load {
                            pointer: ValueId(100),
                            access: private,
                        },
                    ),
                    Operation::new(
                        vec![],
                        Kind::Store {
                            pointer: ValueId(7),
                            value: ValueId(101),
                            access: MemoryAccess::new(AddressSpace::Global, 8),
                        },
                    ),
                ],
                jump(45, &[]),
            ),
            block(
                45,
                vec![],
                vec![Operation::checked_binary(
                    ValueDef::new(ValueId(5), ty),
                    ValueDef::new(ValueId(6), Type::BOOL),
                    CheckedBinaryOperator::Add,
                    ValueId(3),
                    ValueId(2),
                )],
                jump(20, &[5]),
            ),
            block(50, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
    )
}

fn private_cell() -> Function {
    let ty = Type::Scalar(ScalarType::U32);
    let store = || {
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(10),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        )
    };
    Function::internal_helper(
        "private_cell",
        Signature::new(vec![ty.clone()], vec![]),
        vec![ValueId(0)],
        vec![block(
            83,
            vec![],
            vec![
                Operation::effect_free(
                    ValueDef::new(
                        ValueId(10),
                        Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                    ),
                    Kind::Alloca {
                        element: ty,
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 4,
                    },
                ),
                store(),
                store(),
            ],
            Terminator::Return { values: vec![] },
        )],
    )
}

// Only this uncalled helper has a loop. Its conditional entry edge requires H,
// and the live total integer Not can move to H's new preheader. This exercises
// module history, not conditional-loop admission or a genuine source/proof join.
fn helper_motion() -> Function {
    let ty = Type::Scalar(ScalarType::U32);
    Function::internal_helper(
        "helper_motion",
        Signature::new(
            vec![
                ty.clone(),
                ty.clone(),
                Type::pointer(ty.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![
            block(
                10,
                vec![],
                vec![
                    Operation::effect_free(
                        ValueDef::new(ValueId(10), ty.clone()),
                        Kind::Constant(Constant::U32(0)),
                    ),
                    Operation::effect_free(
                        ValueDef::new(ValueId(11), ty.clone()),
                        Kind::Constant(Constant::U32(1)),
                    ),
                ],
                Terminator::ConditionalBranch {
                    condition: ValueId(3),
                    then_target: BlockId(20),
                    then_arguments: vec![ValueId(10)],
                    else_target: BlockId(60),
                    else_arguments: vec![],
                },
            ),
            block(
                20,
                vec![ValueDef::new(ValueId(20), ty.clone())],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(21), Type::BOOL),
                    Kind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(20),
                        rhs: ValueId(0),
                    },
                )],
                Terminator::ConditionalBranch {
                    condition: ValueId(21),
                    then_target: BlockId(30),
                    then_arguments: vec![],
                    else_target: BlockId(60),
                    else_arguments: vec![],
                },
            ),
            block(
                30,
                vec![],
                vec![
                    Operation::effect_free(
                        ValueDef::new(ValueId(30), ty.clone()),
                        Kind::Unary {
                            op: UnaryOp::Not,
                            operand: ValueId(1),
                        },
                    ),
                    Operation::new(
                        vec![],
                        Kind::Store {
                            pointer: ValueId(2),
                            value: ValueId(30),
                            access: MemoryAccess::new(AddressSpace::Global, 4),
                        },
                    ),
                    Operation::effect_free(
                        ValueDef::new(ValueId(35), ty),
                        Kind::Binary {
                            op: BinaryOp::Add,
                            lhs: ValueId(20),
                            rhs: ValueId(11),
                        },
                    ),
                ],
                jump(20, &[35]),
            ),
            block(60, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
    )
}

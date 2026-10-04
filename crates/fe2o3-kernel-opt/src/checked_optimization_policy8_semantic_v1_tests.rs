//! Real component execution supplies rows; portable success is not execution authority.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirBlockSegmentV1, CanonicalKirBlockTransitionV1, CanonicalKirDefinitionDescendantV1,
    CanonicalKirDefinitionTransitionV1, CanonicalKirEdgeArgumentTransitionV1,
    CanonicalKirEdgeTransitionV1, CanonicalKirFunctionTransitionV1,
    CanonicalKirOperationOriginV1 as Origin, CanonicalKirOperationTransitionV1,
    CanonicalKirUseTransitionV1, Constant, Function, MemoryAccess, Module, Operation,
    OperationKind as Kind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
use fe2o3_pliron::{
    OwnedCommutativeBitwiseContinuationV1 as Tail,
    prepare_owned_commutative_bitwise_continuation_v1 as prepare,
};

pub(super) const WORK: usize = 1_000_000_000_000;
pub(super) const STORAGE: usize = 2_000_000_000;
pub(super) const FLOOR: usize = 113;
pub(super) struct Fixture {
    pub(super) input: Owner,
    pub(super) tail: Tail,
    pub(super) backing: usize,
}
pub(super) fn own(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, storage.retained_storage())
}
pub(super) fn fixture(module: &Module) -> Fixture {
    let (input, input_storage) = own(module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input_storage + FLOOR).unwrap();
    let tail = prepare(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), input_storage + FLOOR);
    let backing = input_storage + tail.retained_storage();
    Fixture {
        input,
        tail,
        backing,
    }
}
pub(super) fn claims<'a>(
    input: &Owner,
    output: &Owner,
    rows: Candidate<'a>,
) -> CanonicalPolicy8ContinuationClaimsV1<'a> {
    CanonicalPolicy8ContinuationClaimsV1 {
        pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
        input: InertCanonicalKirTransitionGraphIdentityV1::from_verified(
            input.canonical().identity(),
        ),
        output: InertCanonicalKirTransitionGraphIdentityV1::from_verified(
            output.canonical().identity(),
        ),
        occurrences: rows,
    }
}
pub(super) fn actual_claims(f: &Fixture) -> CanonicalPolicy8ContinuationClaimsV1<'_> {
    claims(&f.input, f.tail.output(), f.tail.occurrences().candidate())
}
fn expression(id: u32, ty: ScalarType, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ty)),
        Kind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn ret(block: &mut BasicBlock, value: u32) {
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(value)],
    });
}
fn module(ty: ScalarType, blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("portable-commutative-subject");
    let ty = Type::Scalar(ty);
    module.functions.push(Function::internal_helper(
        "ordinary_component_function",
        Signature::new(vec![ty.clone(), ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1)],
        blocks,
    ));
    module
}
pub(super) fn pair(ty: ScalarType, op: BinaryOp, cross: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations.push(expression(2, ty, op, 0, 1));
    let duplicate = expression(3, ty, op, 1, 0);
    if cross {
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(20),
            arguments: vec![],
        });
        let mut next = BasicBlock::new(BlockId(20));
        next.operations.push(duplicate);
        ret(&mut next, 3);
        module(ty, vec![entry, next])
    } else {
        entry.operations.push(duplicate);
        ret(&mut entry, 3);
        module(ty, vec![entry])
    }
}
pub(super) fn effects() -> Module {
    let mut input = pair(ScalarType::U32, BinaryOp::BitAnd, false);
    input.functions[0].signature.parameters.push(Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = input.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(4));
    body.blocks[0]
        .operations
        .push(expression(5, ScalarType::U32, BinaryOp::Divide, 0, 1));
    body.blocks[0].operations.push(Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(4),
            value: ValueId(3),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(6), Type::BOOL),
        Kind::Constant(Constant::Bool(true)),
    ));
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(20),
        then_arguments: vec![ValueId(3)],
        else_target: BlockId(20),
        else_arguments: vec![ValueId(3)],
    });
    let mut join = BasicBlock::new(BlockId(20));
    join.parameters
        .push(ValueDef::new(ValueId(7), Type::Scalar(ScalarType::U32)));
    ret(&mut join, 7);
    body.blocks.push(join);
    input
}
fn check_with_floor<'i, 'o, 'r>(
    input: &'i Owner,
    output: &'o Owner,
    claims: CanonicalPolicy8ContinuationClaimsV1<'r>,
    floor: usize,
) -> Result<CheckedCanonicalPolicy8ContinuationRelationV1<'i, 'o, 'r>, Error> {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result =
        check_canonical_policy8_continuation_relation_v1(input, output, claims, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(budget.work() >= 17);
    result
}
fn success(f: &Fixture) -> CheckedCanonicalPolicy8ContinuationRelationV1<'_, '_, '_> {
    check_with_floor(
        &f.input,
        f.tail.output(),
        actual_claims(f),
        FLOOR + f.backing,
    )
    .unwrap()
}
fn refused(f: &Fixture, rows: Candidate<'_>, extra: usize) {
    assert!(matches!(
        check_with_floor(
            &f.input,
            f.tail.output(),
            claims(&f.input, f.tail.output(), rows),
            FLOOR + f.backing + extra,
        ),
        Err(Error::Continuation(_))
    ));
}

#[test]
fn actual_swaps_all_fixed_widths_operators_and_both_block_layouts() {
    for ty in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for op in [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor] {
            for cross in [false, true] {
                let f = fixture(&pair(ty, op, cross));
                let bytes = f.input.canonical().canonical_bytes().to_vec();
                let result = success(&f);
                assert_eq!(result.proved_pairs(), 1);
                assert!(result.has_substitutions());
                assert!(f.tail.execution().changed());
                assert!(std::ptr::eq(result.input(), &f.input));
                assert!(std::ptr::eq(result.output(), f.tail.output()));
                assert_eq!(
                    result.claims().occurrences.operations,
                    f.tail.occurrences().candidate().operations
                );
                assert_ne!(
                    result.input().canonical().canonical_bytes(),
                    result.output().canonical().canonical_bytes()
                );
                assert_eq!(f.input.canonical().canonical_bytes(), bytes);
                assert!(!result.authenticates_execution());
                assert!(!result.grants_authority());
            }
        }
    }
}

#[test]
fn empty_declaration_and_noncommutative_noops_are_zero_pair_relations() {
    let mut empty = Module::new("empty-portable-subject");
    for declarations in [false, true] {
        if declarations {
            empty.functions.push(Function::declaration(
                "external",
                Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
            ));
        }
        let f = fixture(&empty);
        let result = success(&f);
        assert_eq!(result.proved_pairs(), 0);
        assert!(!result.has_substitutions());
        assert!(!f.tail.execution().changed());
        assert_eq!(
            result.input().canonical().canonical_bytes(),
            result.output().canonical().canonical_bytes()
        );
    }
    let f = fixture(&pair(ScalarType::U32, BinaryOp::Add, true));
    let result = success(&f);
    assert_eq!(result.proved_pairs(), 0);
    assert!(!result.claims().occurrences.operations.is_empty());
    assert!(!result.has_substitutions());
    let mut claims = actual_claims(&f);
    claims.occurrences.operations = &[];
    assert!(matches!(
        check_with_floor(&f.input, f.tail.output(), claims, FLOOR + f.backing),
        Err(Error::Continuation(_))
    ));
}

#[test]
fn independently_admitted_equal_bytes_keep_supplied_borrows_not_execution_custody() {
    let source = pair(ScalarType::U32, BinaryOp::BitOr, true);
    let f = fixture(&source);
    let (j, js) = own(&source);
    let (k, ks) = own(f.tail.output().module());
    assert!(!std::ptr::eq(&j, &f.input));
    assert!(!std::ptr::eq(&k, f.tail.output()));
    let result = check_with_floor(&j, &k, actual_claims(&f), FLOOR + f.backing + js + ks).unwrap();
    assert!(std::ptr::eq(result.input(), &j));
    assert!(std::ptr::eq(result.output(), &k));
    assert_eq!(result.proved_pairs(), 1);
    assert!(!result.authenticates_execution());
}

#[test]
fn exact_pass_and_endpoint_claims_are_not_authority() {
    let f = fixture(&pair(ScalarType::U32, BinaryOp::BitXor, true));
    for name in ["", "gpu-commutative-bitwise-dominance-cse-v2"] {
        let mut candidate = actual_claims(&f);
        candidate.pass_name = name;
        assert!(matches!(
            check_with_floor(&f.input, f.tail.output(), candidate, FLOOR + f.backing),
            Err(Error::PassIdentity)
        ));
    }
    let mut candidate = actual_claims(&f);
    candidate.input = candidate.output;
    assert!(matches!(
        check_with_floor(&f.input, f.tail.output(), candidate, FLOOR + f.backing),
        Err(Error::InputIdentity)
    ));
    let mut candidate = actual_claims(&f);
    candidate.output = candidate.input;
    assert!(matches!(
        check_with_floor(&f.input, f.tail.output(), candidate, FLOOR + f.backing),
        Err(Error::OutputIdentity)
    ));
    // Reframing the locators cannot make the reversed actual pair a CSE relation.
    assert!(matches!(
        check_with_floor(
            f.tail.output(),
            &f.input,
            claims(f.tail.output(), &f.input, f.tail.occurrences().candidate()),
            FLOOR + f.backing
        ),
        Err(Error::Continuation(_))
    ));
}

#[derive(Clone)]
struct Rows {
    functions: Vec<CanonicalKirFunctionTransitionV1>,
    blocks: Vec<CanonicalKirBlockTransitionV1>,
    segments: Vec<CanonicalKirBlockSegmentV1>,
    operations: Vec<CanonicalKirOperationTransitionV1>,
    definitions: Vec<CanonicalKirDefinitionTransitionV1>,
    definition_outputs: Vec<CanonicalKirDefinitionDescendantV1>,
    uses: Vec<CanonicalKirUseTransitionV1>,
    edges: Vec<CanonicalKirEdgeTransitionV1>,
    edge_arguments: Vec<CanonicalKirEdgeArgumentTransitionV1>,
}
impl Rows {
    fn new(rows: Candidate<'_>) -> Self {
        Self {
            functions: rows.functions.to_vec(),
            blocks: rows.blocks.to_vec(),
            segments: rows.segments.to_vec(),
            operations: rows.operations.to_vec(),
            definitions: rows.definitions.to_vec(),
            definition_outputs: rows.definition_outputs.to_vec(),
            uses: rows.uses.to_vec(),
            edges: rows.edges.to_vec(),
            edge_arguments: rows.edge_arguments.to_vec(),
        }
    }
    fn candidate(&self) -> Candidate<'_> {
        Candidate {
            functions: &self.functions,
            blocks: &self.blocks,
            segments: &self.segments,
            operations: &self.operations,
            definitions: &self.definitions,
            definition_outputs: &self.definition_outputs,
            uses: &self.uses,
            edges: &self.edges,
            edge_arguments: &self.edge_arguments,
        }
    }
    fn storage(&self) -> usize {
        size_of::<Self>()
            + self.functions.capacity() * size_of::<CanonicalKirFunctionTransitionV1>()
            + self.blocks.capacity() * size_of::<CanonicalKirBlockTransitionV1>()
            + self.segments.capacity() * size_of::<CanonicalKirBlockSegmentV1>()
            + self.operations.capacity() * size_of::<CanonicalKirOperationTransitionV1>()
            + self.definitions.capacity() * size_of::<CanonicalKirDefinitionTransitionV1>()
            + self.definition_outputs.capacity() * size_of::<CanonicalKirDefinitionDescendantV1>()
            + self.uses.capacity() * size_of::<CanonicalKirUseTransitionV1>()
            + self.edges.capacity() * size_of::<CanonicalKirEdgeTransitionV1>()
            + self.edge_arguments.capacity() * size_of::<CanonicalKirEdgeArgumentTransitionV1>()
    }
}

#[test]
fn all_nine_rosters_reject_omissions_extras_duplicates_and_reordering() {
    let mut module = effects();
    let mut second = module.functions[0].clone();
    second.id = "separate_actual_function".into();
    module.functions.push(second);
    let f = fixture(&module);
    assert_eq!(success(&f).proved_pairs(), 2);
    let original = Rows::new(f.tail.occurrences().candidate());
    macro_rules! distort {
        ($field:ident) => {
            assert!(original.$field.len() >= 2);
            for mode in 0..4 {
                let mut rows = original.clone();
                match mode {
                    0 => {
                        rows.$field.pop();
                    }
                    1 => rows.$field.push(rows.$field[0]),
                    2 => rows.$field[1] = rows.$field[0],
                    3 => rows.$field.swap(0, 1),
                    _ => unreachable!(),
                }
                refused(&f, rows.candidate(), rows.storage());
            }
        };
    }
    distort!(functions);
    distort!(blocks);
    distort!(segments);
    distort!(operations);
    distort!(definitions);
    distort!(definition_outputs);
    distort!(uses);
    distort!(edges);
    distort!(edge_arguments);
}

#[test]
fn foreign_origin_and_coordinate_claims_reach_independent_checker() {
    let f = fixture(&effects());
    let original = Rows::new(f.tail.occurrences().candidate());
    let mut rows = original.clone();
    rows.operations[0].origin = rows.operations[1].origin;
    refused(&f, rows.candidate(), rows.storage());
    let mut rows = original.clone();
    let Origin::Retained(mut coordinate) = rows.operations[0].origin else {
        unreachable!()
    };
    coordinate.operation = u32::MAX;
    rows.operations[0].origin = Origin::Retained(coordinate);
    refused(&f, rows.candidate(), rows.storage());
    let mut rows = original;
    rows.definitions[0].outputs.start = u32::MAX;
    refused(&f, rows.candidate(), rows.storage());
}

#[test]
fn reframed_output_changes_cannot_hide_noncommutative_effect_or_cfg_edits() {
    let f = fixture(&effects());
    assert_eq!(success(&f).proved_pairs(), 1);
    for mode in 0..4 {
        let mut graph = f.tail.output().module().clone();
        let body = graph.functions[0].body.as_mut().unwrap();
        match mode {
            0 => {
                let Kind::Binary { op, .. } = &mut body.blocks[0].operations[1].kind else {
                    unreachable!()
                };
                assert_eq!(*op, BinaryOp::Divide);
                *op = BinaryOp::Remainder;
            }
            1 => {
                let Kind::Store { value, .. } = &mut body.blocks[0].operations[2].kind else {
                    unreachable!()
                };
                *value = ValueId(0);
            }
            2 => {
                body.blocks[0].terminator = Some(Terminator::Branch {
                    target: BlockId(20),
                    arguments: vec![ValueId(2)],
                })
            }
            3 => {
                body.blocks[0].operations.remove(1);
            }
            _ => unreachable!(),
        };
        let (changed, storage) = own(&graph);
        assert!(matches!(
            check_with_floor(
                &f.input,
                &changed,
                claims(&f.input, &changed, f.tail.occurrences().candidate()),
                FLOOR + f.backing + storage
            ),
            Err(Error::Continuation(_))
        ));
    }
}

#[test]
fn changed_input_producer_or_stale_output_cannot_pass_with_updated_locators() {
    let module = pair(ScalarType::U32, BinaryOp::BitOr, true);
    let f = fixture(&module);
    let mut changed = module;
    let Kind::Binary { op, .. } =
        &mut changed.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *op = BinaryOp::BitXor;
    let (changed, storage) = own(&changed);
    assert!(matches!(
        check_with_floor(
            &changed,
            f.tail.output(),
            claims(&changed, f.tail.output(), f.tail.occurrences().candidate()),
            FLOOR + f.backing + storage
        ),
        Err(Error::Continuation(_))
    ));
    assert!(matches!(
        check_with_floor(
            &f.input,
            &f.input,
            claims(&f.input, &f.input, f.tail.occurrences().candidate()),
            FLOOR + f.backing
        ),
        Err(Error::Continuation(_))
    ));
}

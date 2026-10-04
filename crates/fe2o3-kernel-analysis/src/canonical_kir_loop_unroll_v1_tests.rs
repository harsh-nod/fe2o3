use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function, Module, Operation,
    Signature, ValueDef,
};
pub(super) const W: usize = 1_000_000_000;
pub(super) const S: usize = 1024 * 1024 * 1024;
pub(super) fn module(n: u32) -> Module {
    let mut m = Module::new("unroll-check-noop");
    let mut b = BasicBlock::new(BlockId(99));
    b.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(77), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(n)),
    ));
    b.terminator = Some(Terminator::Return { values: vec![] });
    m.functions.push(Function::internal_helper(
        "plain",
        Signature::new(vec![], vec![]),
        vec![],
        vec![b],
    ));
    m
}
pub(super) fn admit(m: &Module) -> (Owner, usize) {
    let mut w = Work::new(W);
    let mut b = Budget::new(&mut w, S);
    let (o, s) = Owner::from_module_ref_with_verification_budget_v12(m, &mut b).unwrap();
    (o, s.retained_storage())
}
pub(super) struct TestRows {
    blocks: Vec<Row<Block>>,
    defs: Vec<Row<Definition>>,
    ops: Vec<Row<Site>>,
    terms: Vec<Row<Block>>,
    edges: Vec<Row<Edge>>,
    args: Vec<Row<Argument>>,
}
impl TestRows {
    pub(super) fn identity(owner: &Owner) -> Self {
        let mut w = Work::new(W);
        let mut b = Budget::new(&mut w, S);
        let (i, s) = Inventory::derive(owner, &mut b).unwrap();
        b.reserve_storage(s.retained_storage()).unwrap();
        let out = Self {
            blocks: i
                .blocks()
                .iter()
                .map(|r| Row {
                    input: r.coordinate,
                    output: Some(r.coordinate),
                    copy: CopyRole::Retained,
                })
                .collect(),
            defs: i
                .definitions()
                .iter()
                .map(|r| Row {
                    input: r.coordinate,
                    output: Some(r.coordinate),
                    copy: CopyRole::Retained,
                })
                .collect(),
            ops: i
                .operations()
                .iter()
                .map(|r| Row {
                    input: r.coordinate,
                    output: Some(r.coordinate),
                    copy: CopyRole::Retained,
                })
                .collect(),
            terms: i
                .blocks()
                .iter()
                .map(|r| Row {
                    input: r.coordinate,
                    output: Some(r.coordinate),
                    copy: CopyRole::Retained,
                })
                .collect(),
            edges: i
                .edges()
                .iter()
                .map(|r| Row {
                    input: r.coordinate,
                    output: Some(r.coordinate),
                    copy: CopyRole::Retained,
                })
                .collect(),
            args: i
                .edge_arguments()
                .iter()
                .map(|r| Row {
                    input: r.coordinate,
                    output: Some(r.coordinate),
                    copy: CopyRole::Retained,
                })
                .collect(),
        };
        drop(i);
        b.release_storage(s.retained_storage()).unwrap();
        out
    }
    pub(super) fn view(&self) -> Origins<'_> {
        Origins {
            selection: None,
            blocks: &self.blocks,
            definitions: &self.defs,
            operations: &self.ops,
            terminators: &self.terms,
            edges: &self.edges,
            arguments: &self.args,
        }
    }
    pub(super) fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.blocks.capacity() * size_of::<Row<Block>>()
            + self.defs.capacity() * size_of::<Row<Definition>>()
            + self.ops.capacity() * size_of::<Row<Site>>()
            + self.terms.capacity() * size_of::<Row<Block>>()
            + self.edges.capacity() * size_of::<Row<Edge>>()
            + self.args.capacity() * size_of::<Row<Argument>>()
    }
}
#[test]
fn bounded_unroll_checker_identity_preserves_actual_borrows_and_complete_rows() {
    let (a, sa) = admit(&module(7));
    let (b, sb) = admit(&module(7));
    let rows = TestRows::identity(&a);
    let floor = sa + sb + rows.bytes();
    let mut w = Work::new(W);
    let mut budget = Budget::new(&mut w, S);
    budget.reserve_storage(floor).unwrap();
    let bytes = {
        let (pair, s) = check_canonical_kir_loop_unroll_pair_v1(
            &a,
            &b,
            rows.view(),
            Limits::default(),
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(s.retained_storage()).unwrap();
        assert!(std::ptr::eq(pair.input(), &a));
        assert!(std::ptr::eq(pair.output(), &b));
        assert_eq!(pair.origins().selection, None);
        assert!(!pair.grants_authority());
        s.retained_storage()
    };
    budget.release_storage(bytes).unwrap();
    assert_eq!(budget.storage(), floor);
}
#[test]
fn bounded_unroll_checker_actual_admitted_noop_payload_mutation_refuses() {
    let (a, sa) = admit(&module(7));
    let (b, sb) = admit(&module(8));
    let rows = TestRows::identity(&a);
    let mut w = Work::new(W);
    let mut budget = Budget::new(&mut w, S);
    let floor = sa + sb + rows.bytes();
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        check_canonical_kir_loop_unroll_pair_v1(
            &a,
            &b,
            rows.view(),
            Limits::default(),
            &mut budget
        ),
        Err(Error::Mismatch("outside operation"))
    ));
    assert_eq!(budget.storage(), floor);
}
#[test]
fn bounded_unroll_checker_missing_extra_and_false_copy_rows_refuse() {
    let (a, sa) = admit(&module(7));
    let (b, sb) = admit(&module(7));
    let mut rows = TestRows::identity(&a);
    rows.ops.reserve_exact(1);
    let mut w = Work::new(W);
    let mut budget = Budget::new(&mut w, S);
    let floor = sa + sb + rows.bytes();
    budget.reserve_storage(floor).unwrap();
    let saved = rows.ops[0];
    rows.ops[0].copy = CopyRole::Header(0);
    assert!(matches!(
        check_canonical_kir_loop_unroll_pair_v1(
            &a,
            &b,
            rows.view(),
            Limits::default(),
            &mut budget
        ),
        Err(Error::Mismatch(_))
    ));
    rows.ops[0] = saved;
    rows.ops.push(saved);
    assert!(matches!(
        check_canonical_kir_loop_unroll_pair_v1(
            &a,
            &b,
            rows.view(),
            Limits::default(),
            &mut budget
        ),
        Err(Error::Mismatch("unused origin tail"))
    ));
    rows.ops.clear();
    assert!(matches!(
        check_canonical_kir_loop_unroll_pair_v1(
            &a,
            &b,
            rows.view(),
            Limits::default(),
            &mut budget
        ),
        Err(Error::Mismatch(_))
    ));
    assert_eq!(budget.storage(), floor);
}

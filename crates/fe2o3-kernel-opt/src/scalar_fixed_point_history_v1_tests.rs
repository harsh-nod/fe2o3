use super::*;
pub(crate) use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CheckedBinaryOperator, ComparePredicate, Constant, Function, MemoryAccess, Module, Operation,
    OperationKind as Kind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
pub(crate) const W: usize = 1_000_000_000;
pub(crate) const S: usize = MAX_LOOP_UNROLL_HISTORY_STORAGE_V1;
pub(crate) const SIBLING: usize = 43;

pub(crate) fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut b).unwrap();
    assert_eq!(b.storage(), 0);
    (owner, receipt.retained_storage())
}
pub(crate) fn op(id: u32, ty: Type, kind: Kind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
pub(crate) fn branch(id: u32, args: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(id),
        arguments: args.iter().copied().map(ValueId).collect(),
    }
}
pub(crate) fn block(id: u32, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(terminator);
    block
}
pub(crate) fn reverse_chain(length: usize) -> Module {
    assert!((1..=64).contains(&length));
    let ty = Type::Scalar(ScalarType::U32);
    let binary = |id, lhs, rhs| {
        op(
            id,
            ty.clone(),
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    };
    let mut entry = block(10, branch(100, &[]));
    entry
        .operations
        .push(op(2, ty.clone(), Kind::Constant(Constant::U32(0))));
    let result = 3 + length as u32;
    let mut use_block = block(
        20,
        Terminator::Return {
            values: vec![ValueId(result)],
        },
    );
    use_block.operations.push(binary(result, 1, result - 1));
    let mut blocks = vec![entry, use_block];
    for index in (0..length).rev() {
        let id = 3 + index as u32;
        let next = if index + 1 == length {
            20
        } else {
            101 + index as u32
        };
        let mut producer = block(100 + index as u32, branch(next, &[]));
        producer
            .operations
            .push(binary(id, if index == 0 { 2 } else { id - 1 }, 2));
        blocks.push(producer);
    }
    let mut module = Module::new("scalar-history-reverse-chain");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![ty.clone()], vec![ty]),
        vec![ValueId(1)],
        blocks,
    ));
    module
}
pub(crate) fn effect_module(constant: Constant) -> Module {
    let ty = constant.ty();
    let width = match ty {
        Type::Scalar(ScalarType::F64 | ScalarType::U64) => 8,
        _ => 4,
    };
    let mut entry = block(17, Terminator::Return { values: vec![] });
    entry.operations = vec![
        op(2, ty.clone(), Kind::Constant(constant)),
        op(
            3,
            ty.clone(),
            Kind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(1),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, width),
            },
        ),
    ];
    let mut module = Module::new("scalar-history-strict-effects");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                ty.clone(),
                Type::pointer(ty, AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    module
}
pub(crate) fn produced(module: &Module) -> (Owner, usize, InertScalarFixedPointHistoryBytesV1) {
    let (input, retained) = admit(module);
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(retained + SIBLING).unwrap();
    let core = crate::prepare_checked_scalar_fixed_point_v1(&input, &mut b).unwrap();
    let paid = core.retained_storage();
    b.reserve_storage(paid).unwrap();
    let wire = encode_scalar_fixed_point_history_v1(&input, &core, &mut b).unwrap();
    assert_eq!(b.storage(), retained + SIBLING + paid);
    drop(core);
    b.release_storage(paid).unwrap();
    (input, retained, wire)
}
pub(crate) fn frame(wire: &[u8]) -> InertScalarFixedPointHistoryRefV1<'_> {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(wire.len() + SIBLING).unwrap();
    let frame = read_scalar_fixed_point_history_v1(wire, &mut b).unwrap();
    assert_eq!(b.storage(), wire.len() + SIBLING);
    frame
}
// An independent literal wire constructor is also used to make structurally
// sound but semantically hostile middle/terminal rows in the decode tests.
pub(crate) fn raw(execution: &[u8], rounds: &[[&[u8]; 5]]) -> Vec<u8> {
    let total = 160 + 48 * rounds.len() + rounds.iter().flatten().map(|p| p.len()).sum::<usize>();
    let mut bytes = vec![0; 160 + 48 * rounds.len()];
    bytes[..16].copy_from_slice(b"F2SPH1\0\0\x01\0\x01\0\xa0\0\0\0");
    bytes[16..24].copy_from_slice(&(total as u64).to_le_bytes());
    bytes[24..26].copy_from_slice(&(rounds.len() as u16).to_le_bytes());
    bytes[32..160].copy_from_slice(execution);
    bytes[44..46].copy_from_slice(&(rounds.len() as u16).to_le_bytes());
    for (i, round) in rounds.iter().enumerate() {
        let at = 160 + i * 48;
        bytes[at..at + 2].copy_from_slice(&(i as u16).to_le_bytes());
        bytes[at + 2..at + 4].copy_from_slice(&1u16.to_le_bytes());
        for (j, field) in round.iter().enumerate() {
            bytes[at + 8 + 8 * j..at + 16 + 8 * j]
                .copy_from_slice(&(field.len() as u64).to_le_bytes());
            bytes.extend_from_slice(field);
        }
    }
    assert_eq!(bytes.len(), total);
    bytes
}
pub(crate) fn fields<'w>(frame: &InertScalarFixedPointHistoryRefV1<'w>) -> Vec<[&'w [u8]; 5]> {
    frame.rounds().iter().map(|r| r.fields).collect()
}
pub(crate) fn reference_capacity<T>(n: usize) -> usize {
    let mut reference = Vec::<T>::new();
    reference.try_reserve_exact(n).unwrap();
    reference.capacity() * size_of::<T>()
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Child {
    pub work: usize,
    pub peak: usize,
    pub retained: usize,
}
pub(crate) fn child(floor: usize, f: impl FnOnce(&mut Budget<'_>) -> usize) -> Child {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(floor).unwrap();
    let retained = f(&mut b);
    assert_eq!(b.storage(), floor);
    Child {
        work: b.work(),
        peak: b.peak_storage() - floor,
        retained,
    }
}
#[derive(Debug)]
pub(crate) struct Trace {
    pub work: usize,
    pub live: usize,
    pub peak: usize,
}
impl Trace {
    pub fn new(floor: usize, header: usize) -> Self {
        Self {
            work: 0,
            live: floor + header,
            peak: floor + header,
        }
    }
    pub fn reserve(&mut self, n: usize) {
        self.live += n;
        self.peak = self.peak.max(self.live);
    }
    pub fn release(&mut self, n: usize) {
        self.live -= n;
    }
    pub fn work(&mut self, n: usize) {
        self.work += n;
    }
    pub fn table<T>(&mut self, n: usize) -> usize {
        self.work(4);
        let capacity = reference_capacity::<T>(n);
        self.reserve(capacity);
        capacity
    }
    pub fn child(&mut self, child: Child) {
        self.work(child.work);
        self.peak = self.peak.max(self.live + child.peak);
        self.reserve(child.retained);
    }
}
pub(crate) fn read_cost(count: usize) -> Child {
    Child {
        work: 160 + 48 * count + 1,
        peak: size_of::<Meter<'_, '_>>()
            + size_of::<InertScalarFixedPointHistoryRefV1<'_>>()
            + size_of::<[usize; 5]>()
            + 2 * size_of::<usize>(),
        retained: size_of::<InertScalarFixedPointHistoryRefV1<'_>>(),
    }
}

#[test]
fn closed_scalar_wire_roundtrips_every_row_and_terminal_graph_range() {
    for module in [
        Module::new("empty-history"),
        reverse_chain(1),
        reverse_chain(8),
    ] {
        let (_, _, wire) = produced(&module);
        let framed = frame(wire.canonical_bytes());
        let literal = raw(framed.execution_claim(), &fields(&framed));
        assert_eq!(literal, wire.canonical_bytes());
        assert_eq!(&literal[framed.final_graph_range()], framed.output_bytes());
        assert!(!framed.grants_authority());
        if !module.functions.is_empty() {
            assert!(framed.rounds().len() >= 3);
        }
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        let floor =
            wire.storage().retained_storage() + framed.storage().retained_storage() + SIBLING;
        b.reserve_storage(floor).unwrap();
        let encoded = reencode_scalar_fixed_point_history_v1(&framed, &mut b).unwrap();
        assert_eq!(encoded.canonical_bytes(), literal);
        assert_eq!(
            encoded.storage().retained_storage(),
            size_of::<InertScalarFixedPointHistoryBytesV1>()
                + reference_capacity::<u8>(literal.len())
        );
        assert_eq!(b.storage(), floor);
    }
}

#[test]
fn scalar_wire_rejects_domains_roles_reserved_lengths_truncation_and_surplus() {
    let (_, _, wire) = produced(&reverse_chain(1));
    let bytes = wire.canonical_bytes();
    let mut cases = vec![];
    for at in [
        0, 8, 10, 12, 16, 26, 32, 40, 42, 44, 46, 128, 136, 144, 160, 162, 164,
    ] {
        let mut changed = bytes.to_vec();
        changed[at] ^= 0x80;
        cases.push(changed);
    }
    for field in 0..5 {
        let mut changed = bytes.to_vec();
        changed[168 + 8 * field..176 + 8 * field].copy_from_slice(&u64::MAX.to_le_bytes());
        cases.push(changed);
    }
    for count in [0u16, 17] {
        let mut changed = bytes.to_vec();
        changed[24..26].copy_from_slice(&count.to_le_bytes());
        cases.push(changed);
    }
    cases.push(bytes[..159].to_vec());
    cases.push(bytes[..bytes.len() - 1].to_vec());
    let mut extra = bytes.to_vec();
    extra.push(0);
    cases.push(extra);
    for bytes in cases {
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        b.reserve_storage(bytes.len() + SIBLING).unwrap();
        assert!(read_scalar_fixed_point_history_v1(&bytes, &mut b).is_err());
        assert_eq!(b.storage(), bytes.len() + SIBLING);
    }
}

#[test]
fn scalar_reader_exact_independent_work_peak_and_first_denial_markers() {
    let (_, _, wire) = produced(&reverse_chain(1));
    let bytes = wire.canonical_bytes();
    let n = u16::from_le_bytes(bytes[24..26].try_into().unwrap()) as usize;
    let expected = read_cost(n);
    let floor = bytes.len() + SIBLING;
    for (w, s, accepted, failed_w, failed_s, success) in [
        (
            expected.work,
            floor + expected.peak,
            expected.work,
            None,
            None,
            true,
        ),
        (159, floor + expected.peak, 0, Some(160), None, false),
        (
            160 + 48 * n - 1,
            floor + expected.peak,
            160,
            Some(160 + 48 * n),
            None,
            false,
        ),
        (
            expected.work - 1,
            floor + expected.peak,
            expected.work - 1,
            Some(expected.work),
            None,
            false,
        ),
        (
            expected.work,
            floor + expected.peak - 1,
            0,
            None,
            Some(floor + expected.peak),
            false,
        ),
    ] {
        let mut work = Work::new(w);
        {
            let mut b = Budget::new(&mut work, s);
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let result = read_scalar_fixed_point_history_v1(bytes, &mut b);
            assert_eq!(result.is_ok(), success);
            if success {
                assert_eq!(b.peak_storage(), floor + expected.peak);
            } else {
                assert!(matches!(result, Err(Error::Resource(_))));
            }
            assert_eq!(
                (b.work(), b.failed_storage(), b.storage()),
                (accepted, failed_s, floor)
            );
            assert!(b.work_ledger_identity_v1() == ledger);
        }
        assert_eq!(work.failed_work(), failed_w);
    }
}

#[test]
fn scalar_codec_keeps_prior_denial_markers_and_checked_arithmetic() {
    let (_, _, wire) = produced(&Module::new("prior-denials"));
    let floor = wire.canonical_bytes().len() + SIBLING;
    let mut work = Work::new(W);
    {
        let mut b = Budget::new(&mut work, S);
        b.reserve_storage(floor).unwrap();
        assert!(b.charge_work(W + 1).is_err());
        assert!(b.reserve_storage(S + 1).is_err());
        let expected = read_cost(1);
        let frame = read_scalar_fixed_point_history_v1(wire.canonical_bytes(), &mut b).unwrap();
        assert_eq!(frame.rounds().len(), 1);
        assert_eq!(
            (b.work(), b.storage(), b.failed_storage()),
            (expected.work, floor, Some(floor + S + 1))
        );
    }
    assert_eq!(work.failed_work(), Some(W + 1));
    assert!(matches!(
        add(usize::MAX, 1),
        Err(Error::Resource(Resource::Arithmetic))
    ));
}

fn writer_trace(frame: &InertScalarFixedPointHistoryRefV1<'_>, trace: &mut Trace) -> usize {
    trace.reserve(
        size_of::<InertScalarFixedPointHistoryBytesV1>()
            + size_of::<[usize; 5]>()
            + 2 * size_of::<usize>(),
    );
    trace.work(frame.rounds().len() * 5 + 1);
    let capacity = trace.table::<u8>(frame.canonical_bytes().len());
    trace.work(frame.canonical_bytes().len() * 2);
    let read = read_cost(frame.rounds().len());
    trace.child(read);
    trace.work(1);
    trace.release(read.retained);
    size_of::<InertScalarFixedPointHistoryBytesV1>() + capacity
}

#[test]
fn scalar_encoder_and_reencoder_use_independent_child_composition_not_observed_peaks() {
    let (input, input_paid) = admit(&reverse_chain(1));
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(input_paid + SIBLING).unwrap();
    let core = crate::prepare_checked_scalar_fixed_point_v1(&input, &mut b).unwrap();
    let floor = input_paid + SIBLING + core.retained_storage();
    b.reserve_storage(core.retained_storage()).unwrap();
    let wire = encode_scalar_fixed_point_history_v1(&input, &core, &mut b).unwrap();
    let framed = frame(wire.canonical_bytes());
    let mut trace = Trace::new(floor, size_of::<Meter<'_, '_>>());
    trace.reserve(
        size_of::<Vec<EncodedRound>>() + size_of::<[InertScalarFixedPointRoundRefV1<'_>; 16]>(),
    );
    trace.child(child(trace.live, |b| {
        core.replay_against(&input, b).unwrap();
        0
    }));
    trace.table::<EncodedRound>(core.rounds().len());
    let mut before = &input;
    for round in core.rounds() {
        trace.work(1);
        trace.child(child(trace.live, |b| {
            let (_, paid) = Transition::from_candidate_with_budget(
                before.canonical().identity(),
                round.integer().owner().canonical().identity(),
                round.integer().occurrences().candidate(),
                b,
            )
            .unwrap();
            paid.retained_storage()
        }));
        trace.child(child(trace.live, |b| {
            encode_checked_canonical_policy3_execution_receipt_v1(
                round.integer().owner(),
                round.scalar(),
                b,
            )
            .unwrap()
            .storage()
            .retained_storage()
        }));
        trace.work(1);
        before = round.output();
    }
    let retained = writer_trace(&framed, &mut trace);
    for limit in [trace.work, trace.work - 1] {
        let mut work = Work::new(limit);
        {
            let mut b = Budget::new(&mut work, trace.peak);
            b.reserve_storage(floor).unwrap();
            let result = encode_scalar_fixed_point_history_v1(&input, &core, &mut b);
            match result {
                Ok(value) => {
                    assert_eq!(limit, trace.work);
                    assert_eq!(value.storage().retained_storage(), retained);
                    assert_eq!(value.canonical_bytes(), wire.canonical_bytes());
                }
                Err(Error::Resource(Resource::Work(_))) => assert_eq!(limit, trace.work - 1),
                Err(e) => panic!("{e}"),
            }
            assert_eq!(
                (b.work(), b.peak_storage(), b.storage()),
                (limit, trace.peak, floor)
            );
        }
        assert_eq!(
            work.failed_work(),
            if limit == trace.work {
                None
            } else {
                Some(trace.work)
            }
        );
    }
    let floor = wire.storage().retained_storage() + framed.storage().retained_storage() + SIBLING;
    let mut trace = Trace::new(floor, size_of::<Meter<'_, '_>>());
    writer_trace(&framed, &mut trace);
    let mut work = Work::new(trace.work);
    let mut b = Budget::new(&mut work, trace.peak);
    b.reserve_storage(floor).unwrap();
    assert_eq!(
        reencode_scalar_fixed_point_history_v1(&framed, &mut b)
            .unwrap()
            .canonical_bytes(),
        wire.canonical_bytes()
    );
    assert_eq!(
        (b.work(), b.peak_storage(), b.storage()),
        (trace.work, trace.peak, floor)
    );
}

#[test]
fn scalar_wire_limits_and_prepaid_floor_are_not_implicitly_raised() {
    let (_, _, wire) = produced(&Module::new("floor"));
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S + 1);
    assert!(matches!(
        read_scalar_fixed_point_history_v1(wire.canonical_bytes(), &mut b),
        Err(Error::Limit)
    ));
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(wire.canonical_bytes().len() - 1).unwrap();
    assert!(matches!(
        read_scalar_fixed_point_history_v1(wire.canonical_bytes(), &mut b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(b.work(), 0);
    let child_bound = 160
        + 16 * (2 * MAX_MODULE_BYTES_V1
            + 416
            + MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1
            + MAX_CANONICAL_POLICY3_EXECUTION_RECEIPT_BYTES_V1
            + 48);
    assert_eq!(MAX_SCALAR_FIXED_POINT_HISTORY_BYTES_V1, child_bound);
    assert!(child_bound > 160 * 1024 * 1024);
}

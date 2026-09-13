use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Function, MemoryElementType,
    MemoryIntrinsicOperation, MemoryLayout, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, VolatileAccessContract,
};

const WORK_PREFIX: usize = 11;
const STORAGE_PREFIX: usize = 7;

struct Probe {
    result: Result<
        (
            VerifiedCanonicalKernelIrV12,
            CanonicalKernelIrReplayStorageV12,
        ),
        CanonicalKernelIrReplayAdmissionErrorV12,
    >,
    work: usize,
    peak: usize,
    rejected_work: Option<usize>,
    rejected_storage: Option<usize>,
}

fn admit(source: &Module, work_allowance: usize, storage_allowance: usize) -> Probe {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK_PREFIX + work_allowance);
    work.charge_work(WORK_PREFIX).unwrap();
    let (result, peak, rejected_storage) = {
        let mut resources = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            STORAGE_PREFIX + storage_allowance,
        );
        resources.reserve_storage(STORAGE_PREFIX).unwrap();
        let result = VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
            source,
            &mut resources,
        );
        assert_eq!(resources.storage(), STORAGE_PREFIX);
        (result, resources.peak_storage(), resources.failed_storage())
    };
    Probe {
        result,
        work: work.work(),
        peak,
        rejected_work: work.failed_work(),
        rejected_storage,
    }
}

fn borrowed_source() -> Module {
    let mut identifier = String::with_capacity(4_096);
    identifier.push('m');
    Module::new(identifier)
}

fn nested_type_source() -> Module {
    let mut module = borrowed_source();
    module.functions.push(Function::declaration(
        "f",
        Signature::new(
            vec![Type::pointer(
                Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            )],
            vec![],
        ),
    ));
    module.functions.reserve(17);
    module.functions[0].signature.parameters.reserve(19);
    module
}

fn volatile_source(instances: usize) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    for index in 0..instances {
        block.operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(u32::try_from(index + 1).unwrap()),
                Type::Scalar(ScalarType::U32),
            ),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(0),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space: AddressSpace::Global,
                layout: MemoryLayout::new(4, 4),
                contract: VolatileAccessContract::rust_allocation_load(),
            }),
        ));
    }
    block.operations.reserve(23);
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = borrowed_source();
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            )],
            vec![],
        ),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

fn owner_payload(wire_bytes: usize) -> usize {
    std::mem::size_of::<VerifiedCanonicalKernelIrV12>() + wire_bytes
}

#[test]
fn full_owner_nested_pointer_slice_boxes_have_independent_storage_boundaries() {
    let source = nested_type_source();
    let snapshot = source.clone();
    let source_capacity = source.id.retained_capacity_bytes();

    // Module37 + function ID5 + signature counts8 + pointer/slice/scalar8 +
    // absent body1 + function capability count4 = 63 bytes, 24 extent tokens.
    // One function, no kernels: role census1 + comparison-width margin2 = 3.
    const WIRE: usize = 63;
    const COUNT: usize = 24;
    const ROLES: usize = 3;
    const ENCODED: usize = COUNT + (COUNT + ROLES + WIRE + 4);
    // The decoder has read through the scalar before either enclosing Box is
    // allocated: wire offset54 + the two IDs' UTF-8/copy work4.
    const BOX_WORK: usize = ENCODED + 54 + 4;
    // Fresh inverse: wire63 + ID validation/copy4 + role restore3 + Count24 +
    // comparison (wire63 + patched length4 + chunk tokens24 + end1 + roles3).
    const PREVERIFY: usize = ENCODED + WIRE + 4 + ROLES + COUNT + WIRE + 4 + COUNT + 1 + ROLES;
    assert_eq!((ENCODED, BOX_WORK, PREVERIFY), (118, 176, 307));
    assert_eq!(encode_module_v12(&source).unwrap().len(), WIRE);
    verify_module(&source).unwrap();

    // The signature Vec owns its Type row; each recursive pointee/element adds
    // one separate Box<Type>. Source Vec/String spare capacities are excluded.
    let before_boxes = owner_payload(WIRE)
        + std::mem::size_of::<Module>()
        + std::mem::size_of::<Function>()
        + 2
        + std::mem::size_of::<Type>();
    let box_bytes = std::mem::size_of::<Type>();
    for already_allocated in 0..2 {
        let rejected = before_boxes + (already_allocated + 1) * box_bytes;
        let probe = admit(&source, PREVERIFY, rejected - 1);
        assert!(matches!(
            probe.result,
            Err(CanonicalKernelIrReplayAdmissionErrorV12::Decode(KernelIrDecodeError::Resource(
                CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
            ))) if error.actual() == STORAGE_PREFIX + rejected
                && error.limit() == STORAGE_PREFIX + rejected - 1
        ));
        assert_eq!(probe.work, WORK_PREFIX + BOX_WORK);
        assert_eq!(
            probe.peak,
            STORAGE_PREFIX + before_boxes + already_allocated * box_bytes
        );
        assert_eq!(probe.rejected_storage, Some(STORAGE_PREFIX + rejected));
        assert_eq!(probe.rejected_work, None);
    }

    let inverse_payload = before_boxes + 2 * box_bytes;
    for allowance in [PREVERIFY - 1, PREVERIFY] {
        let probe = admit(&source, allowance, inverse_payload);
        if allowance == PREVERIFY - 1 {
            // The final comparison token is rejected with all decoded boxes live.
            assert!(matches!(
                probe.result,
                Err(CanonicalKernelIrReplayAdmissionErrorV12::Decode(KernelIrDecodeError::Encode(
                    KernelIrEncodeError::WorkLimit(error)
                ))) if error.actual() == WORK_PREFIX + PREVERIFY
            ));
        } else {
            // Exact inverse budget reaches the semantic function-index census,
            // whose first token must fail before it can reserve verifier scratch.
            assert!(matches!(
                probe.result,
                Err(CanonicalKernelIrReplayAdmissionErrorV12::Resource(
                    CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                )) if error.actual() == WORK_PREFIX + PREVERIFY + 1
            ));
        }
        assert_eq!(probe.work, WORK_PREFIX + allowance);
        assert_eq!(probe.rejected_work, Some(WORK_PREFIX + allowance + 1));
        assert_eq!(probe.peak, STORAGE_PREFIX + inverse_payload);
        assert_eq!(probe.rejected_storage, None);
    }
    assert_eq!(source, snapshot);
    assert_eq!(source.id.retained_capacity_bytes(), source_capacity);
}

#[test]
fn full_owner_precharges_peak_intrinsic_producer_storage_before_encoder_allocation() {
    // Volatile payload: seven tags + size8 + alignment4 = 19.
    // The framed instance is header20 + payload19 = 39. Both coexist: 58.
    const PRODUCER: usize = 19 + 39;
    for instances in [1, 3] {
        let source = volatile_source(instances);
        let snapshot = source.clone();
        // Empty helper81 + pointer parameter type5 + body parameter ID4.
        // Each load: result roster4 + ValueDef6 + tag1 + length4 + instance39 + operand4.
        let wire = 90 + 58 * instances;
        // Base helper24 + pointer type5 + body ID1; each load has eight schema
        // tokens and retains its 58-unit producer-work precharge even in Count.
        let count = 30 + (8 + PRODUCER) * instances;
        let retained = owner_payload(wire);
        assert_eq!(encode_module_v12(&source).unwrap().len(), wire);
        verify_module(&source).unwrap();

        for scratch in [PRODUCER - 1, PRODUCER] {
            let probe = admit(&source, count, retained + scratch);
            if scratch == PRODUCER - 1 {
                assert!(matches!(
                    probe.result,
                    Err(CanonicalKernelIrReplayAdmissionErrorV12::Resource(
                        CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
                    )) if error.actual() == STORAGE_PREFIX + retained + PRODUCER
                ));
                assert_eq!(probe.peak, STORAGE_PREFIX + retained);
                assert_eq!(
                    probe.rejected_storage,
                    Some(STORAGE_PREFIX + retained + PRODUCER)
                );
                assert_eq!(probe.rejected_work, None);
            } else {
                // Count consumed its exact work. Encoder Count's first schema
                // token now fails, after admission of wire plus producer scratch.
                assert!(matches!(
                    probe.result,
                    Err(CanonicalKernelIrReplayAdmissionErrorV12::Encode(
                        KernelIrEncodeError::WorkLimit(error)
                    )) if error.actual() == WORK_PREFIX + count + 1
                ));
                assert_eq!(probe.peak, STORAGE_PREFIX + retained + PRODUCER);
                assert_eq!(probe.rejected_storage, None);
                assert_eq!(probe.rejected_work, Some(WORK_PREFIX + count + 1));
            }
            assert_eq!(probe.work, WORK_PREFIX + count);
        }
        assert_eq!(source, snapshot);
    }
}

#[test]
fn full_owner_inverse_and_intrinsic_comparison_scratch_must_coexist() {
    const PRODUCER: usize = 19 + 39;
    const ROLES: usize = 3;
    for instances in [1, 3] {
        let source = volatile_source(instances);
        let snapshot = source.clone();
        let wire = 90 + 58 * instances;
        let count = 30 + (8 + PRODUCER) * instances;
        let encoded = count + (count + ROLES + wire + PRODUCER * instances + 4);
        // Decoder read/copy, 4*39 validation work per semantic instance, role
        // restoration, then allocation-free extent counting before comparison.
        let before_scratch_work = encoded + wire + 4 + 4 * 39 * instances + ROLES + count;
        assert_eq!(before_scratch_work, 284 + 528 * instances);
        let inverse_payload = owner_payload(wire)
            + std::mem::size_of::<Module>()
            + std::mem::size_of::<Function>()
            + 2
            + 2 * std::mem::size_of::<Type>()
            + std::mem::size_of::<ValueId>()
            + std::mem::size_of::<BasicBlock>()
            + instances * (std::mem::size_of::<Operation>() + std::mem::size_of::<ValueDef>());

        for scratch in [PRODUCER - 1, PRODUCER] {
            let probe = admit(&source, before_scratch_work, inverse_payload + scratch);
            if scratch == PRODUCER - 1 {
                assert!(matches!(
                    probe.result,
                    Err(CanonicalKernelIrReplayAdmissionErrorV12::Decode(KernelIrDecodeError::Resource(
                        CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
                    ))) if error.actual() == STORAGE_PREFIX + inverse_payload + PRODUCER
                ));
                assert_eq!(probe.peak, STORAGE_PREFIX + inverse_payload);
                assert_eq!(
                    probe.rejected_storage,
                    Some(STORAGE_PREFIX + inverse_payload + PRODUCER)
                );
                assert_eq!(probe.rejected_work, None);
            } else {
                // The comparison's first eight-byte magic chunk is rejected;
                // decoder ownership and the producer reservation must both exist.
                assert!(matches!(
                    probe.result,
                    Err(CanonicalKernelIrReplayAdmissionErrorV12::Decode(KernelIrDecodeError::Encode(
                        KernelIrEncodeError::WorkLimit(error)
                    ))) if error.actual() == WORK_PREFIX + before_scratch_work + 8
                ));
                assert_eq!(probe.peak, STORAGE_PREFIX + inverse_payload + PRODUCER);
                assert_eq!(probe.rejected_storage, None);
                assert_eq!(
                    probe.rejected_work,
                    Some(WORK_PREFIX + before_scratch_work + 8)
                );
            }
            assert_eq!(probe.work, WORK_PREFIX + before_scratch_work);
        }
        assert_eq!(source, snapshot);
    }
}

#[test]
fn full_owner_payload_fixtures_complete_without_retaining_source_owners() {
    // This generous fixed envelope is only a completion check, not an oracle
    // for the independently derived phase boundaries in the preceding tests.
    for source in [nested_type_source(), volatile_source(1), volatile_source(3)] {
        let snapshot = source.clone();
        let source_capacity = source.id.retained_capacity_bytes();
        let expected = VerifiedCanonicalKernelIrV12::from_module(snapshot.clone()).unwrap();
        let probe = admit(&source, 1_000_000, 1_000_000);
        let (owner, receipt) = probe.result.unwrap();
        assert_eq!(owner, expected);
        assert_eq!(
            receipt.retained_storage(),
            owner_payload(owner.canonical_bytes().len())
        );
        assert!(probe.peak >= STORAGE_PREFIX + receipt.retained_storage());
        assert_eq!(probe.rejected_work, None);
        assert_eq!(probe.rejected_storage, None);
        assert_eq!(source, snapshot);
        assert_eq!(source.id.retained_capacity_bytes(), source_capacity);
        drop(source);
        owner.revalidate().unwrap();
    }
}

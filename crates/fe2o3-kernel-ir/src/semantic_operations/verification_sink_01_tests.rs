use std::fmt;

use super::*;
use crate::{
    Constant, Convergence, MatrixFrontendBindingV2, MatrixOperation, MatrixOperationIssueSinkV1,
    MatrixProjectedKernargPolicyV1, MatrixProviderIdentityV2, MatrixSourceAbiObservationV2,
    MatrixVerificationIssueKind, SynchronizationScope, TensorLayoutContractV1,
    TensorLayoutFindingV1, TensorOperandRoleV1, verify_tensor_layout_contract_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SinkFailure {
    Work,
    Storage,
}

#[derive(Default)]
struct FormatLength(usize);

impl fmt::Write for FormatLength {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = self.0.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

fn formatted_length(arguments: fmt::Arguments<'_>) -> usize {
    let mut length = FormatLength::default();
    fmt::write(&mut length, arguments).expect("test message length fits usize");
    length.0
}

struct SemanticBoundarySink {
    work_limit: usize,
    storage_limit: usize,
    work: usize,
    storage: usize,
    issues: Vec<SemanticOperationIssue>,
}

impl SemanticBoundarySink {
    fn new(work_limit: usize, storage_limit: usize) -> Self {
        Self {
            work_limit,
            storage_limit,
            work: 0,
            storage: 0,
            issues: Vec::new(),
        }
    }
}

impl SemanticOperationIssueSinkV1 for SemanticBoundarySink {
    type Error = SinkFailure;

    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        let required = self.work.checked_add(amount).ok_or(SinkFailure::Work)?;
        if required > self.work_limit {
            return Err(SinkFailure::Work);
        }
        self.work = required;
        Ok(())
    }

    fn emit(
        &mut self,
        kind: SemanticOperationIssueKind,
        message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), Self::Error> {
        self.charge_work(message_work_upper)?;
        let message_length = formatted_length(arguments);
        let required = self
            .storage
            .checked_add(1 + message_length)
            .ok_or(SinkFailure::Storage)?;
        if required > self.storage_limit {
            return Err(SinkFailure::Storage);
        }
        let message = arguments.to_string();
        self.storage = required;
        self.issues.push(SemanticOperationIssue { kind, message });
        Ok(())
    }
}

struct MatrixBoundarySink {
    work_limit: usize,
    storage_limit: usize,
    work: usize,
    storage: usize,
    issues: Vec<(MatrixVerificationIssueKind, String)>,
}

impl MatrixBoundarySink {
    fn new(work_limit: usize, storage_limit: usize) -> Self {
        Self {
            work_limit,
            storage_limit,
            work: 0,
            storage: 0,
            issues: Vec::new(),
        }
    }
}

impl MatrixOperationIssueSinkV1 for MatrixBoundarySink {
    type Error = SinkFailure;

    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        let required = self.work.checked_add(amount).ok_or(SinkFailure::Work)?;
        if required > self.work_limit {
            return Err(SinkFailure::Work);
        }
        self.work = required;
        Ok(())
    }

    fn allocate_coordinates(&mut self, count: usize) -> Result<Vec<[u64; 2]>, Self::Error> {
        self.charge_work(6)?;
        let cells = count.checked_mul(2).ok_or(SinkFailure::Storage)?;
        let required = self
            .storage
            .checked_add(cells)
            .ok_or(SinkFailure::Storage)?;
        if required > self.storage_limit {
            return Err(SinkFailure::Storage);
        }
        let mut coordinates = Vec::new();
        coordinates
            .try_reserve_exact(count)
            .map_err(|_| SinkFailure::Storage)?;
        self.storage = required;
        Ok(coordinates)
    }

    fn release_coordinates(
        &mut self,
        coordinates: Vec<[u64; 2]>,
        count: usize,
    ) -> Result<(), Self::Error> {
        drop(coordinates);
        self.storage = self
            .storage
            .checked_sub(count * 2)
            .ok_or(SinkFailure::Storage)?;
        Ok(())
    }

    fn emit(
        &mut self,
        kind: MatrixVerificationIssueKind,
        message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), Self::Error> {
        self.charge_work(message_work_upper)?;
        let message_length = formatted_length(arguments);
        let required = self
            .storage
            .checked_add(1 + message_length)
            .ok_or(SinkFailure::Storage)?;
        if required > self.storage_limit {
            return Err(SinkFailure::Storage);
        }
        let message = arguments.to_string();
        self.storage = required;
        self.issues.push((kind, message));
        Ok(())
    }
}

fn invalid_copy() -> MemoryIntrinsicOperation {
    let element = MemoryElementType::Scalar(ScalarType::U32);
    MemoryIntrinsicOperation::CopyNonOverlapping {
        source: ValueId(0),
        destination: ValueId(1),
        count: ValueId(2),
        element,
        source_address_space: AddressSpace::Global,
        destination_address_space: AddressSpace::Global,
        layout: MemoryLayout::new(8, 4),
        contract: CopyNonOverlappingContract::supported_rust(),
    }
}

fn invalid_copy_context() -> SemanticOperationVerificationContext<'static> {
    let operands = Box::leak(Box::new([ValueId(0), ValueId(1), ValueId(2)]));
    let results = Box::leak(Box::new([ValueDef::new(ValueId(3), Type::Unit)]));
    let operand_types = Box::leak(Box::new([
        Some(Type::F32),
        Some(Type::pointer(
            Type::F32,
            AddressSpace::Global,
            AccessMode::ReadOnly,
        )),
        Some(Type::F32),
    ]));
    SemanticOperationVerificationContext {
        operands,
        results,
        operand_types,
    }
}

#[test]
fn semantic_dispatch_is_inert_for_nonsemantic_operations() {
    let mut sink = SemanticBoundarySink::new(0, 0);
    assert_eq!(
        try_verify_semantic_operation_with_sink_v1(
            &OperationKind::Constant(Constant::Index(0)),
            SemanticOperationBorrowedVerificationContextV1 {
                operands: &[],
                results: &[],
                operand_types: &[],
            },
            &mut sink,
        ),
        Ok(false)
    );
    assert_eq!((sink.work, sink.storage, sink.issues.len()), (0, 0, 0));
}

#[test]
fn valid_intrinsic_uses_no_issue_storage() {
    let operation = IntrinsicOperation::launch_extent_1d();
    let result = ValueDef::new(ValueId(0), Type::INDEX);
    let context = SemanticOperationBorrowedVerificationContextV1 {
        operands: &[],
        results: std::slice::from_ref(&result),
        operand_types: &[],
    };
    let mut sink = SemanticBoundarySink::new(usize::MAX, 0);
    assert_eq!(
        try_verify_intrinsic_with_sink_v1(&operation, context, &mut sink),
        Ok(())
    );
    assert_eq!(sink.storage, 0);
    assert!(sink.work > 0);
}

#[test]
fn memory_sink_preserves_legacy_issue_order_and_exact_boundaries() {
    let operation = invalid_copy();
    let context = invalid_copy_context();
    let legacy = operation.verify(context);
    let borrowed_operand_types = context
        .operand_types
        .iter()
        .map(Option::as_ref)
        .collect::<Vec<_>>();
    let borrowed_context = SemanticOperationBorrowedVerificationContextV1 {
        operands: context.operands,
        results: context.results,
        operand_types: &borrowed_operand_types,
    };
    let mut observed = SemanticBoundarySink::new(usize::MAX, usize::MAX);
    try_verify_memory_intrinsic_with_sink_v1(&operation, borrowed_context, &mut observed).unwrap();
    assert_eq!(observed.issues, legacy);
    assert_eq!(
        observed
            .issues
            .iter()
            .map(|issue| issue.kind)
            .collect::<Vec<_>>(),
        vec![
            SemanticOperationIssueKind::ResultArity,
            SemanticOperationIssueKind::InvalidStructure,
            SemanticOperationIssueKind::InvalidOperandType,
            SemanticOperationIssueKind::InvalidOperandType,
            SemanticOperationIssueKind::InvalidOperandType,
        ]
    );
    const POINTER_REQUIRED: &str = "memory intrinsic requires a pointer operand";
    let expected_messages = [
        "operation defines 1 results but 0 are required".to_string(),
        "memory layout MemoryLayout { size_bytes: 8, alignment_bytes: 4 } does not match the closed Scalar(U32) layout MemoryLayout { size_bytes: 4, alignment_bytes: 4 }".to_string(),
        POINTER_REQUIRED.to_string(),
        "pointer operand PointerType { pointee: Scalar(F32), address_space: Global, access: ReadOnly } does not match element Scalar(U32), address space Global, writable true".to_string(),
        "memory intrinsic operand has type Scalar(F32), expected Scalar(Index)".to_string(),
    ];
    assert_eq!(
        observed
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>(),
        expected_messages
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    // 17 fixed/type visits, two generic 512-byte bounds, one fixed pointer
    // message, a 1,152-byte pointer bound, and a 1,792-byte type bound.
    let exact_work = 17 + 2 * 512 + POINTER_REQUIRED.len() + 1_152 + 1_792;
    let exact_storage = expected_messages
        .iter()
        .map(|message| 1 + message.len())
        .sum::<usize>();
    assert_eq!(
        (observed.work, observed.storage),
        (exact_work, exact_storage)
    );
    let mut exact = SemanticBoundarySink::new(exact_work, exact_storage);
    assert_eq!(
        try_verify_memory_intrinsic_with_sink_v1(&operation, borrowed_context, &mut exact),
        Ok(())
    );
    let mut work_one_under = SemanticBoundarySink::new(exact_work - 1, usize::MAX);
    assert_eq!(
        try_verify_memory_intrinsic_with_sink_v1(&operation, borrowed_context, &mut work_one_under),
        Err(SinkFailure::Work)
    );
    let mut storage_one_under = SemanticBoundarySink::new(usize::MAX, exact_storage - 1);
    assert_eq!(
        try_verify_memory_intrinsic_with_sink_v1(
            &operation,
            borrowed_context,
            &mut storage_one_under,
        ),
        Err(SinkFailure::Storage)
    );
    assert!(!storage_one_under.issues.is_empty());
}

#[test]
fn rich_invalid_matrix_has_deterministic_sink_boundaries() {
    const CONVERGENCE: &str = "matrix V1 requires uniform subgroup convergence";
    const LDS_LAYOUT: &str = "matrix LDS operations cannot carry an instruction layout contract";
    const LDS_BASE: &str = "matrix LDS base must be a workgroup pointer";
    let mut operation = MatrixOperation::lds_load(ValueId(0), crate::MatrixElement::Bf16)
        .with_declared_tensor_layout(
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
        );
    operation.active_lanes = 32;
    operation.convergence = Convergence::uniform(SynchronizationScope::Workgroup);
    let operand_types = [Some(Type::F32)];
    let borrowed_operand_types = [operand_types[0].as_ref()];
    let results = [ValueDef::new(ValueId(1), Type::INDEX)];
    let legacy = operation.verify(&operand_types, &results);
    let mut observed = MatrixBoundarySink::new(usize::MAX, usize::MAX);
    operation
        .try_verify_with_sink_v1(&borrowed_operand_types, &results, &mut observed)
        .unwrap();
    assert_eq!(
        observed.issues,
        legacy
            .iter()
            .map(|issue| (issue.kind, issue.message.clone()))
            .collect::<Vec<_>>()
    );
    let expected_messages = [
        "matrix V1 requires all 64 lanes active, found 32".to_string(),
        CONVERGENCE.to_string(),
        LDS_LAYOUT.to_string(),
        LDS_BASE.to_string(),
        "matrix operation defines 1 results, expected 4".to_string(),
        "matrix result %1 has type Scalar(Index), expected Scalar(Bf16)".to_string(),
    ];
    assert_eq!(
        observed
            .issues
            .iter()
            .map(|(_, message)| message.as_str())
            .collect::<Vec<_>>(),
        expected_messages
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    // 14 fixed/type visits, two 512-byte numeric bounds, three exact fixed
    // messages, and one 1,792-byte two-type result bound.
    let exact_work = 14 + 2 * 512 + CONVERGENCE.len() + LDS_LAYOUT.len() + LDS_BASE.len() + 1_792;
    let exact_storage = expected_messages
        .iter()
        .map(|message| 1 + message.len())
        .sum::<usize>();
    assert_eq!(
        (observed.work, observed.storage),
        (exact_work, exact_storage)
    );
    let mut exact = MatrixBoundarySink::new(exact_work, exact_storage);
    assert_eq!(
        operation.try_verify_with_sink_v1(&borrowed_operand_types, &results, &mut exact),
        Ok(())
    );
    let mut work_one_under = MatrixBoundarySink::new(exact_work - 1, usize::MAX);
    assert_eq!(
        operation.try_verify_with_sink_v1(&borrowed_operand_types, &results, &mut work_one_under,),
        Err(SinkFailure::Work)
    );
    let mut storage_one_under = MatrixBoundarySink::new(usize::MAX, exact_storage - 1);
    assert_eq!(
        operation.try_verify_with_sink_v1(
            &borrowed_operand_types,
            &results,
            &mut storage_one_under,
        ),
        Err(SinkFailure::Storage)
    );
}

#[test]
fn canonical_tensor_coordinates_preserve_subgroup_width_findings() {
    let incomplete = [
        TensorLayoutFindingV1::IncompleteCoverage {
            role: TensorOperandRoleV1::A,
        },
        TensorLayoutFindingV1::IncompleteCoverage {
            role: TensorOperandRoleV1::B,
        },
        TensorLayoutFindingV1::IncompleteCoverage {
            role: TensorOperandRoleV1::Accumulator,
        },
    ];
    for subgroup_width in [0, 32] {
        let mut contract = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
        contract.subgroup_width = subgroup_width;
        let mut expected = vec![TensorLayoutFindingV1::ProfileMismatch {
            field: "subgroup width",
        }];
        expected.extend(incomplete.iter().cloned());
        assert_eq!(verify_tensor_layout_contract_v1(&contract), expected);
    }
    let canonical = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    assert!(verify_tensor_layout_contract_v1(&canonical).is_empty());
    let mut wider = canonical;
    wider.subgroup_width = 65;
    assert_eq!(
        verify_tensor_layout_contract_v1(&wider),
        vec![TensorLayoutFindingV1::ProfileMismatch {
            field: "subgroup width",
        }]
    );
}

#[test]
fn matrix_capability_visitor_matches_owned_deduplicated_roster() {
    let values4 = [ValueId(0); 4];
    let values8 = [ValueId(0); 8];
    let binding = MatrixFrontendBindingV2 {
        observed_source: MatrixSourceAbiObservationV2 {
            provider: MatrixProviderIdentityV2 {
                crate_name: String::new(),
                stable_crate_id: 0,
                crate_hash: [0; 16],
                cargo_metadata_build_observation: [0; 32],
                source_identity: [0; 32],
                definition_identities: Vec::new(),
            },
            canonical_record: Vec::new(),
            digest: [0x5a; 32],
        },
        projected_kernarg: MatrixProjectedKernargPolicyV1::canonical(),
    };
    let operations = [
        MatrixOperation::multiply_accumulate(values4, values4, values4)
            .with_frontend_binding(binding),
        MatrixOperation::scaled_multiply_accumulate_fp8_e4m3(values8, values8, values4),
        MatrixOperation::scaled_multiply_accumulate_fp4_e2m1(values8, values8, values4),
        MatrixOperation::lds_load(ValueId(0), crate::MatrixElement::Bf16),
        MatrixOperation::lds_load(ValueId(0), crate::MatrixElement::F32),
        MatrixOperation::lds_store(ValueId(0), values4, crate::MatrixElement::Bf16),
    ];
    for operation in operations {
        let expected = operation
            .required_capabilities()
            .into_iter()
            .collect::<Vec<_>>();
        let mut actual = Vec::new();
        operation
            .try_visit_required_capabilities_v1(|capability| {
                actual.push(capability.into_owned());
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
        assert_eq!(actual, expected);
    }
}

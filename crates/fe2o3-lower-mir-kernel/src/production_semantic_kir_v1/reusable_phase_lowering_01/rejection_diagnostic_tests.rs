//! Inert diagnostic/type-consistency tests, not authenticated source fixtures.
use super::*;
use fe2o3_kernel_ir::{
    ExecutionCapabilityRoleV1, ExecutionCapabilitySignatureV1,
    ExecutionCapabilitySourceOccurrenceV1, ExecutionCapabilitySourceV1, ExecutionCapabilityTypeV1,
    ExecutionSafetyObligationsV1, ExecutionTypeIdentityV1, FunctionId, PhaseCallOccurrenceV1,
    PhaseDefinedCallV1, PhaseKeyV1, PhaseLoanStateV1, ReusablePhaseOpV1, ReusablePhaseTokenRoleV1,
    ReusablePhaseTokenTypeV1,
};

fn provenance_fixture() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("phase_diagnostic"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn fixture() -> (ReusablePhaseOpV1, Type) {
    let input = ExecutionTypeIdentityV1::new([11; 32]);
    let output = ExecutionTypeIdentityV1::new([90; 32]);
    let operation = ReusablePhaseOperationV1::OwnerConvert {
        workgroup: input,
        owner: output,
    };
    let contract = ReusablePhaseOpV1 {
        operands: vec![ValueId(1)],
        obligations: ExecutionSafetyObligationsV1::from_bits(operation.required_obligations()),
        operation,
        provenance: provenance_fixture(),
        source: PhaseOperationSourceV1::Defined(PhaseDefinedCallV1 {
            call: PhaseCallOccurrenceV1 {
                source: ExecutionCapabilitySourceV1 {
                    function: [101; 32],
                    operation: [108; 32],
                    block: 8,
                    occurrence: ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                        [99; 32], [100; 32], [101; 32], 1, 0,
                    ),
                },
                callee_instance: 8,
                original_normal_target: 9,
                expanded_normal_target: 0,
            },
            signature: ExecutionCapabilitySignatureV1::new(&[input], output).unwrap(),
            defined_abi: [71; 32],
            defined_body: [72; 32],
            incoming_count: 2,
            incoming_digest: [73; 32],
            source_binding: [74; 32],
        }),
    };
    let ty = Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: input,
        role: ExecutionCapabilityRoleV1::Workgroup,
        provenance: provenance_fixture(),
        workgroup_brand: Some([7; 32]),
        epoch: Some([8; 32]),
    });
    (contract, ty)
}

fn observe(contract: &ReusablePhaseOpV1, inputs: &[(ValueId, &Type)], first: ValueId) -> Buffer {
    render(
        &(2usize, "OwnerConvert", 17u32, Some(3u32)),
        &contract.operation,
        &contract.source,
        &[17u8; 32],
        "phase_diagnostic",
        inputs,
        first,
    )
}

#[test]
fn actual_local_type_gate_remains_exact_before_and_after_observation() {
    let (contract, ty) = fixture();
    let before = contract
        .clone()
        .checked_operation(&[(ValueId(1), &ty)], ValueId(100))
        .unwrap();
    let text = observe(&contract, &[(ValueId(1), &ty)], ValueId(100));
    assert!(!text.truncated);
    assert!(text.text().contains("role=Workgroup"));
    assert!(text.text().contains("source_type=ExecutionTypeIdentityV1"));
    assert!(text.text().contains("before_first_result=true"));
    let after = contract
        .checked_operation(&[(ValueId(1), &ty)], ValueId(100))
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn role_source_provenance_brand_epoch_and_producer_substitutions_still_reject() {
    let (contract, original) = fixture();
    for kind in 0..7 {
        let mut ty = original.clone();
        let Type::ExecutionCapability(cap) = &mut ty else {
            unreachable!()
        };
        let mut id = ValueId(1);
        match kind {
            0 => cap.role = ExecutionCapabilityRoleV1::ReusableWorkgroup,
            1 => cap.source_type = ExecutionTypeIdentityV1::new([12; 32]),
            2 => cap.provenance.issuance = [9; 32],
            3 => cap.workgroup_brand = None,
            4 => cap.epoch = None,
            5 => id = ValueId(2),
            6 => id = ValueId(100),
            _ => unreachable!(),
        }
        let inputs = [(id, &ty)];
        assert!(
            contract
                .clone()
                .checked_operation(&inputs, ValueId(100))
                .is_none()
        );
        let saved = ty.clone();
        let output = observe(&contract, &inputs, ValueId(100));
        assert!(!output.truncated);
        assert_eq!(saved, ty);
        assert!(
            contract
                .clone()
                .checked_operation(&inputs, ValueId(100))
                .is_none()
        );
    }
}

#[test]
fn token_roles_and_root_prefix_are_observed_without_retyping() {
    let (contract, _) = fixture();
    let mut provenance = provenance_fixture();
    provenance.root = FunctionId::new("r".repeat(ROOT_LIMIT + 100));
    let PhaseOperationSourceV1::Defined(defined) = contract.source else {
        unreachable!()
    };
    let ty = Type::ReusablePhaseToken(ReusablePhaseTokenTypeV1 {
        provenance,
        phase: PhaseKeyV1::for_begin(defined.call).unwrap(),
        owner_source: ExecutionTypeIdentityV1::new([8; 32]),
        owner_anchor_epoch: [9; 32],
        outer_brand: [10; 32],
        phase_brand: [11; 32],
        initial_epoch: [12; 32],
        role: ReusablePhaseTokenRoleV1::OwnerLoan(PhaseLoanStateV1::Sealed),
    });
    let saved = ty.clone();
    let output = observe(&contract, &[(ValueId(4), &ty)], ValueId(3));
    assert!(output.text().contains("role=OwnerLoan(Sealed)"));
    assert!(output.text().contains("root_truncated=true"));
    assert!(output.text().contains("before_first_result=false"));
    assert!(!output.text().contains(&"r".repeat(ROOT_LIMIT + 1)));
    assert_eq!(saved, ty);
}

#[test]
fn operand_observation_is_capped_and_does_not_walk_other_types() {
    let (contract, _) = fixture();
    let ty = Type::Unit;
    let inputs = vec![(ValueId(3), &ty); MAX_EXECUTION_CAPABILITY_OPERANDS_V1 + 10];
    let output = observe(&contract, &inputs, ValueId(100));
    assert_eq!(
        output.text().matches("\ninput=").count(),
        MAX_EXECUTION_CAPABILITY_OPERANDS_V1
    );
    assert!(output.text().contains("inputs_truncated=true"));
    assert!(output.text().contains("kind=Other"));
}

#[test]
fn byte_limit_preserves_utf8_and_stops_later_formatting() {
    let mut out = Buffer::new();
    out.write_str(&"x".repeat(BYTE_LIMIT - 1)).unwrap();
    assert!(out.write_str("\u{00e9}").is_err());
    assert!(out.truncated);
    assert_eq!(out.len, BYTE_LIMIT - 1);
    assert!(out.write_str("not retained").is_err());
    assert!(!out.text().contains("not retained"));
    assert!(std::str::from_utf8(&out.bytes[..out.len]).is_ok());
    let mut logged = Vec::new();
    out.publish(&mut logged);
    assert!(logged.ends_with(b" diagnostic_truncated=true\n"));
    assert!(logged.len() <= BYTE_LIMIT + 40);
}

#[test]
fn diagnostic_io_failure_is_ignored_without_touching_the_input() {
    struct Failure;
    impl io::Write for Failure {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    let (contract, ty) = fixture();
    let output = observe(&contract, &[(ValueId(1), &ty)], ValueId(100));
    output.publish(&mut Failure);
    assert!(
        contract
            .checked_operation(&[(ValueId(1), &ty)], ValueId(100))
            .is_some()
    );
}

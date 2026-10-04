//! Synthetic state/shape/resource controls. These do not construct a sealed
//! candidate or claim a genuine caller-dataflow positive; C2/C4 supply that.
use super::super::tensor_capability_read_v1::capability_known_origin_read_v1;
use super::super::{
    AllocationContractV1, ProjectedCapabilityEnumEnvelopeV1, ProjectedCapabilityOriginV1,
    ProjectedCapabilityStateV1, ProjectedCapabilityValueV1, ProjectedMfmaAccumulatorV1,
    ProjectedMfmaOperandV1, SemanticMfmaAccumulatorContractV1,
    SemanticMfmaAccumulatorDistributionV1, SemanticMfmaOperandContractV1,
    SemanticMfmaOperandRoleV1, SemanticMfmaProfileV1, SemanticMfmaRegisterDistributionV1,
    SemanticOperandV1, SemanticPlaceV1, authenticate_tensor_instruction_v1,
};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, TensorLayoutContractV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallDestinationV1, SemanticCallableIdV1, SemanticConstantV1, SemanticConstantValueV1,
    SemanticControlFlowEdgeV1, SemanticLayoutIdentityV1, SemanticProjectionKindV1,
    SemanticProjectionV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};
use std::cell::Cell;

const TY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
fn operand(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], TY).unwrap(),
    )
}
fn call_with(arguments: Vec<SemanticOperandV1>) -> SemanticDirectCallV1 {
    SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(7),
        arguments,
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(4),
                vec![],
                SemanticTypeIdV1::from_index(1),
            )
            .unwrap(),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(9),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
}
fn call() -> SemanticDirectCallV1 {
    call_with((0..4).map(operand).collect())
}
fn operand_contract(role: SemanticMfmaOperandRoleV1) -> SemanticMfmaOperandContractV1 {
    SemanticMfmaOperandContractV1 {
        role,
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
        wave_width: 64,
    }
}
fn accumulator_contract() -> SemanticMfmaAccumulatorContractV1 {
    SemanticMfmaAccumulatorContractV1 {
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
        wave_width: 64,
    }
}
fn required() -> TensorLayoutContractV1 {
    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
        .with_zero_filled_predicate_inputs()
}
fn state() -> ProjectedCapabilityStateV1 {
    let allocation = AllocationContractV1 {
        allocation_origin: 101,
        noalias_class: 102,
        writable: false,
        singleton_object: false,
    };
    ProjectedCapabilityStateV1::from([
        (
            0,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::MatrixContext {
                root: 10,
            }),
        ),
        (
            1,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(
                ProjectedMfmaOperandV1 {
                    contract: operand_contract(SemanticMfmaOperandRoleV1::A),
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                    lane_root: 20,
                    allocation,
                },
            )),
        ),
        (
            2,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(
                ProjectedMfmaOperandV1 {
                    contract: operand_contract(SemanticMfmaOperandRoleV1::B),
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                    lane_root: 20,
                    allocation,
                },
            )),
        ),
        (
            3,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Accumulator(
                ProjectedMfmaAccumulatorV1 {
                    contract: accumulator_contract(),
                    lane_root: 20,
                    value_root: 30,
                    flow_root: 31,
                },
            )),
        ),
    ])
}
struct SyntheticFixedRead {
    values: [Option<ProjectedCapabilityValueV1>; 4],
    reads: Cell<usize>,
}
impl SyntheticFixedRead {
    fn from_map(state: &ProjectedCapabilityStateV1) -> Self {
        Self {
            values: std::array::from_fn(|i| state.get(&i).copied()),
            reads: Cell::new(0),
        }
    }
}
impl CapabilityStateReadV1 for SyntheticFixedRead {
    fn capability_value_v1(&self, local: usize) -> Option<ProjectedCapabilityValueV1> {
        self.reads.set(self.reads.get() + 1);
        self.values.get(local).copied().flatten()
    }
}
fn core(
    call: &SemanticDirectCallV1,
    state: &(impl CapabilityStateReadV1 + ?Sized),
) -> std::result::Result<AuthenticatedTensorInstructionV1, &'static str> {
    authenticate_tensor_instruction_read_v1(
        call,
        state,
        operand_contract(SemanticMfmaOperandRoleV1::A),
        operand_contract(SemanticMfmaOperandRoleV1::B),
        accumulator_contract(),
    )
}
fn mutate_operand(
    state: &mut ProjectedCapabilityStateV1,
    local: usize,
    mutate: impl FnOnce(&mut ProjectedMfmaOperandV1),
) {
    let Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(value))) =
        state.get_mut(&local)
    else {
        panic!("synthetic operand")
    };
    mutate(value);
}

#[test]
fn synthetic_read_adapter_matches_legacy_and_does_not_write_array_destination() {
    let state = state();
    let before = state.clone();
    let fixed = SyntheticFixedRead::from_map(&state);
    let call = call();
    let legacy = authenticate_tensor_instruction_v1(
        &call,
        &state,
        operand_contract(SemanticMfmaOperandRoleV1::A),
        operand_contract(SemanticMfmaOperandRoleV1::B),
        accumulator_contract(),
    );
    assert_eq!(core(&call, &fixed), legacy);
    let actual = legacy.unwrap();
    assert_eq!(actual.context_root, 10);
    assert_eq!(actual.accumulator.value_root, 30); // INPUT, not array output.
    assert_eq!(actual.accumulator.flow_root, 31);
    assert_eq!(actual.contract, required());
    assert_eq!(fixed.reads.get(), 4);
    assert_eq!(state, before);
    assert!(!state.contains_key(&4));
}

#[test]
fn synthetic_missing_context_a_b_acc_have_precise_lattice_reasons() {
    let reasons = [
        "an MFMA call without dominating compiler-issued matrix context",
        "an MFMA lhs without one dominating checked typed-load payload",
        "an MFMA rhs without one dominating checked typed-load payload",
        "an MFMA accumulator without dominating zero or compatible prior MFMA",
    ];
    for (local, reason) in reasons.into_iter().enumerate() {
        let mut state = state();
        state.remove(&local);
        assert_eq!(core(&call(), &state), Err(reason));
        state.insert(local, ProjectedCapabilityValueV1::Invalid);
        assert_eq!(core(&call(), &state), Err(reason));
    }
}

#[test]
fn synthetic_unknown_enum_projected_and_constant_operands_never_become_known() {
    let mut state = state();
    let origin = ProjectedCapabilityOriginV1::MatrixContext { root: 10 };
    state.insert(
        0,
        ProjectedCapabilityValueV1::ConstructedEnum(ProjectedCapabilityEnumEnvelopeV1 {
            origin,
            variants: [0; super::super::MAX_PROJECTED_CAPABILITY_ENUM_DEPTH_V1],
            depth: 1,
        }),
    );
    assert_eq!(capability_known_origin_read_v1(&state, &operand(0)), None);
    state.insert(0, ProjectedCapabilityValueV1::Known(origin));
    let projected = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(0),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), TY).unwrap()],
            TY,
        )
        .unwrap(),
    );
    let constant = SemanticOperandV1::Constant(SemanticConstantV1::new(
        TY,
        SemanticConstantValueV1::ZeroSized,
    ));
    assert_eq!(capability_known_origin_read_v1(&state, &projected), None);
    assert_eq!(capability_known_origin_read_v1(&state, &constant), None);
    assert_eq!(capability_known_origin_read_v1(&state, &operand(99)), None);
}

#[test]
fn synthetic_whole_moves_read_original_caller_slots_without_consuming() {
    let state = state();
    let before = state.clone();
    let args = (0..4)
        .map(|local| {
            SemanticOperandV1::Move(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], TY).unwrap(),
            )
        })
        .collect();
    assert!(core(&call_with(args), &state).is_ok());
    assert_eq!(state, before);
    assert!(core(&call_with(vec![operand(0), operand(1), operand(2)]), &state).is_err());
}

#[test]
fn synthetic_swapped_roles_metadata_and_lane_roots_refuse() {
    let mut swapped = state();
    let a = swapped[&1];
    swapped.insert(1, swapped[&2]);
    swapped.insert(2, a);
    assert_eq!(
        core(&call(), &swapped),
        Err("an MFMA call whose operand metadata does not match its exact load producers")
    );
    let mut wrong_lane = state();
    mutate_operand(&mut wrong_lane, 2, |v| v.lane_root = 999);
    assert_eq!(
        core(&call(), &wrong_lane),
        Err("an MFMA call whose operands do not share one authenticated wave64 lane")
    );
    let mut wrong_profile = state();
    mutate_operand(&mut wrong_profile, 1, |v| {
        v.contract.profile = SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128
    });
    assert_eq!(
        core(&call(), &wrong_profile),
        Err("an MFMA call whose operand metadata does not match its exact load producers")
    );
    let mut wrong_acc = state();
    let Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Accumulator(v))) =
        wrong_acc.get_mut(&3)
    else {
        panic!("synthetic accumulator")
    };
    v.contract.wave_width = 32;
    assert_eq!(
        core(&call(), &wrong_acc),
        Err("an MFMA call whose accumulator metadata changed from its producer")
    );
}

#[test]
fn synthetic_generic_column_major_success_does_not_relax_closed_nominal_storage() {
    for (local, storage) in [
        (1, SemanticMfmaStorageLayoutV1::LdsXor4),
        (2, SemanticMfmaStorageLayoutV1::LdsXor4),
        (2, SemanticMfmaStorageLayoutV1::ColumnMajor),
    ] {
        let mut state = state();
        mutate_operand(&mut state, local, |v| v.storage_layout = storage);
        let tensor = core(&call(), &state).unwrap();
        if storage == SemanticMfmaStorageLayoutV1::ColumnMajor {
            assert_eq!(
                tensor.contract,
                required(),
                "equal register layout is insufficient"
            );
        }
        assert_eq!(
            require_nominal_tensor_contract(tensor, required()),
            Err("a nominal MFMA call outside its exact RowMajor source-storage profile")
        );
        assert_eq!(
            authenticate_tensor_instruction_v1(
                &call(),
                &state,
                operand_contract(SemanticMfmaOperandRoleV1::A),
                operand_contract(SemanticMfmaOperandRoleV1::B),
                accumulator_contract()
            ),
            Ok(tensor)
        );
    }
}

#[test]
fn synthetic_required_contract_does_not_drop_zero_fill_requirement() {
    let tensor = core(&call(), &state()).unwrap();
    assert_eq!(require_nominal_tensor_contract(tensor, required()), Ok(()));
    assert_eq!(
        require_nominal_tensor_contract(
            tensor,
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
        ),
        Err("a nominal MFMA call whose authenticated tensor contract differs")
    );
}

fn type_decl(tag: u8, shape: SemanticTypeShapeV1) -> SemanticTypeDeclV1 {
    // Synthetic shape-only input, not semantic admission or rustc ABI evidence.
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
        shape,
    )
}
#[test]
fn synthetic_result_shape_is_exact_array_of_four_f32_not_nominal_accumulator() {
    let f32_type = || {
        type_decl(
            1,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        )
    };
    let array = |length| {
        type_decl(
            2,
            SemanticTypeShapeV1::Array {
                element: TY,
                length,
            },
        )
    };
    let array_id = SemanticTypeIdV1::from_index(1);
    assert_eq!(
        require_array_type(&[f32_type(), array(4)], array_id),
        Ok(())
    );
    for length in [0, 3, 5] {
        assert_eq!(
            require_array_type(&[f32_type(), array(length)], array_id),
            Err(Error::Unavailable(
                "nominal source result is not a four-element array"
            ))
        );
    }
    assert!(require_array_type(&[f32_type(), array(4)], TY).is_err());
    assert!(
        require_array_type(
            &[f32_type(), type_decl(3, SemanticTypeShapeV1::Opaque)],
            array_id
        )
        .is_err()
    );
    assert!(
        require_array_type(
            &[
                type_decl(
                    1,
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 64 })
                ),
                array(4)
            ],
            array_id
        )
        .is_err()
    );
    assert!(require_array_type(&[], array_id).is_err());
}

#[test]
fn synthetic_original_meter_exact_work_and_one_short_preserve_resource_vs_invalid() {
    let call = call();
    for short in [false, true] {
        let fixed = SyntheticFixedRead::from_map(&state());
        let prefix = 11;
        let limit = prefix + CALLER_TENSOR_WORK_V1 - usize::from(short);
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 23);
        budget.reserve_storage(23).unwrap();
        budget.charge_work(prefix).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let result = authenticate_nominal_tensor_origin(
            &call,
            &fixed,
            operand_contract(SemanticMfmaOperandRoleV1::A),
            operand_contract(SemanticMfmaOperandRoleV1::B),
            accumulator_contract(),
            required(),
            &mut budget,
        );
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), 23);
        assert_eq!(budget.peak_storage(), 23);
        if short {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(fixed.reads.get(), 0);
            assert!(budget.failed_work().is_some());
        } else {
            assert!(matches!(result, Ok(Ok(_))));
            assert_eq!(fixed.reads.get(), 4);
            assert_eq!(budget.work(), prefix + CALLER_TENSOR_WORK_V1);
            assert_eq!(budget.failed_work(), None);
        }
    }
    let empty = SyntheticFixedRead {
        values: [None; 4],
        reads: Cell::new(0),
    };
    let mut work = Work::new(CALLER_TENSOR_WORK_V1);
    let mut budget = Budget::new(&mut work, 0);
    assert!(matches!(
        authenticate_nominal_tensor_origin(
            &call,
            &empty,
            operand_contract(SemanticMfmaOperandRoleV1::A),
            operand_contract(SemanticMfmaOperandRoleV1::B),
            accumulator_contract(),
            required(),
            &mut budget
        ),
        Ok(Err(
            "an MFMA call without dominating compiler-issued matrix context"
        ))
    ));
    assert_eq!(budget.failed_work(), None);
}

#[test]
fn fixed_c1_frames_fit_the_existing_nominal_query_stack_envelope() {
    // C1 allocates no heap or retained output. N2a owns its existing 4096-byte
    // fixed resolver stack envelope. C2 must prepay any retained output table.
    let frames = std::mem::size_of::<NominalCallerSiteV1<'static>>()
        + 2 * std::mem::size_of::<NominalTransferOutcomeV1<'static>>()
        + 2 * std::mem::size_of::<AuthenticatedTensorInstructionV1>()
        + 4 * std::mem::size_of::<Option<ProjectedCapabilityOriginV1>>();
    assert!(frames < 4096);
}

#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Guarded case preparation and fail-closed protected-execution contracts for
//! the issue #138 general tiled GEMM.
//!
//! This package performs no device execution. The authority and argument types
//! intentionally have no constructors until the source, proof, artifact, and
//! protected runtime joins exist.

use core::{fmt, marker::PhantomData, ops::Range};

use fe2o3_amd_target::AmdTargetId;
use fe2o3_general_gemm_compiler::{
    GENERAL_GEMM_DEVICE_TARGET_V1, GeneralGemmArtifactBindingIdentityV1,
    GeneralGemmCompilationBindingIdentityV1, GeneralGemmRuntimeAbiIdentityV1,
    GeneralGemmRuntimeAbiSnapshotV1, GeneralGemmScheduleIdentityV1, GeneralGemmScheduleV1,
};
use fe2o3_tiled_gemm_v1::{
    GeneralGemmPlanV1, GeneralGemmRequestV1, GeneralLaunchLimitsV1, GeneralPlanErrorV1,
    GeneralReferenceErrorV1, admit_target_v1, execute_general_reference_v1, plan_general_gemm_v1,
};
use fe2o3_verifier::GeneralGemmEvidenceIdentityV1;

/// Exact number of adjacent elements checked on each side of every body.
pub const GENERAL_GEMM_GUARD_ELEMENTS_V1: usize = 32;
/// Fixed workgroup required by both schedules.
pub const GENERAL_GEMM_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Exact single-buffered A+B LDS allocation.
pub const GENERAL_GEMM_LDS_BYTES_V1: u32 = 1_024;
/// Number of typed runtime arguments in the safe source ABI.
pub const GENERAL_GEMM_ARGUMENT_COUNT_V1: usize = 11;
/// The protected execution join is intentionally unavailable at this checkpoint.
pub const GENERAL_GEMM_PROTECTED_EXECUTION_AVAILABLE_V1: bool = false;

const A_BODY_POISON: u16 = 0x7fc1;
const A_LEFT_GUARD: u16 = 0x7fc2;
const A_RIGHT_GUARD: u16 = 0x7fc3;
const B_BODY_POISON: u16 = 0x7fd1;
const B_LEFT_GUARD: u16 = 0x7fd2;
const B_RIGHT_GUARD: u16 = 0x7fd3;
const C_BODY_POISON: f32 = f32::from_bits(0x7fc0_c001);
const C_LEFT_GUARD: f32 = f32::from_bits(0x7fc0_c002);
const C_RIGHT_GUARD: f32 = f32::from_bits(0x7fc0_c003);

/// One immutable deterministic hardware case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmHardwareCaseV1 {
    name: &'static str,
    schedule: GeneralGemmScheduleV1,
    dimensions: [u32; 3],
    strides: [u32; 3],
    alpha_bits: u32,
    beta_bits: u32,
}

impl GeneralGemmHardwareCaseV1 {
    const fn new(
        name: &'static str,
        schedule: GeneralGemmScheduleV1,
        dimensions: [u32; 3],
        strides: [u32; 3],
        alpha: f32,
        beta: f32,
    ) -> Self {
        Self {
            name,
            schedule,
            dimensions,
            strides,
            alpha_bits: alpha.to_bits(),
            beta_bits: beta.to_bits(),
        }
    }

    /// Returns the stable case name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the independently qualified schedule.
    pub const fn schedule(self) -> GeneralGemmScheduleV1 {
        self.schedule
    }

    /// Returns `[M, N, K]`.
    pub const fn dimensions(self) -> [u32; 3] {
        self.dimensions
    }

    /// Returns `[lda, ldb, ldc]`.
    pub const fn strides(self) -> [u32; 3] {
        self.strides
    }

    /// Returns exact `[alpha, beta]` FP32 bits.
    pub const fn coefficient_bits(self) -> [u32; 2] {
        [self.alpha_bits, self.beta_bits]
    }
}

const REFERENCE: GeneralGemmScheduleV1 = GeneralGemmScheduleV1::ReferenceWave64Xor4V1;
const VECTOR_A: GeneralGemmScheduleV1 = GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1;

/// Seven independent shape/stride/coefficient profiles for each schedule.
pub const GENERAL_GEMM_HARDWARE_CASES_V1: [GeneralGemmHardwareCaseV1; 14] = [
    GeneralGemmHardwareCaseV1::new(
        "reference-packed",
        REFERENCE,
        [16, 16, 16],
        [16, 16, 16],
        1.0,
        0.0,
    ),
    GeneralGemmHardwareCaseV1::new(
        "reference-strided-all-tails",
        REFERENCE,
        [17, 19, 18],
        [23, 29, 31],
        0.5,
        -1.0,
    ),
    GeneralGemmHardwareCaseV1::new(
        "reference-multi-wg-multi-phase",
        REFERENCE,
        [33, 35, 33],
        [37, 41, 43],
        2.0,
        0.25,
    ),
    GeneralGemmHardwareCaseV1::new(
        "reference-m-tail",
        REFERENCE,
        [17, 16, 16],
        [16, 16, 16],
        -1.0,
        1.0,
    ),
    GeneralGemmHardwareCaseV1::new(
        "reference-n-tail",
        REFERENCE,
        [16, 17, 16],
        [16, 19, 23],
        1.5,
        -0.5,
    ),
    GeneralGemmHardwareCaseV1::new(
        "reference-k-tail",
        REFERENCE,
        [16, 16, 17],
        [21, 19, 16],
        -0.5,
        2.0,
    ),
    GeneralGemmHardwareCaseV1::new(
        "reference-zero-k",
        REFERENCE,
        [17, 19, 0],
        [0, 0, 23],
        7.0,
        0.5,
    ),
    GeneralGemmHardwareCaseV1::new(
        "vector-a-packed",
        VECTOR_A,
        [16, 16, 16],
        [16, 16, 16],
        1.0,
        0.0,
    ),
    GeneralGemmHardwareCaseV1::new(
        "vector-a-strided-all-tails",
        VECTOR_A,
        [17, 19, 18],
        [23, 29, 31],
        0.5,
        -1.0,
    ),
    GeneralGemmHardwareCaseV1::new(
        "vector-a-multi-wg-multi-phase",
        VECTOR_A,
        [33, 35, 33],
        [37, 41, 43],
        2.0,
        0.25,
    ),
    GeneralGemmHardwareCaseV1::new(
        "vector-a-m-tail",
        VECTOR_A,
        [17, 16, 16],
        [16, 16, 16],
        -1.0,
        1.0,
    ),
    GeneralGemmHardwareCaseV1::new(
        "vector-a-n-tail",
        VECTOR_A,
        [16, 17, 16],
        [16, 19, 23],
        1.5,
        -0.5,
    ),
    GeneralGemmHardwareCaseV1::new(
        "vector-a-k-tail",
        VECTOR_A,
        [16, 16, 17],
        [21, 19, 16],
        -0.5,
        2.0,
    ),
    GeneralGemmHardwareCaseV1::new(
        "vector-a-zero-k",
        VECTOR_A,
        [17, 19, 0],
        [0, 0, 23],
        7.0,
        0.5,
    ),
];

/// One owned body with exact adjacent left and right guards.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardedStorageV1<T> {
    allocation: Vec<T>,
    body: Range<usize>,
}

impl<T> GuardedStorageV1<T> {
    /// Returns the complete guard+body+guard allocation.
    pub fn allocation(&self) -> &[T] {
        &self.allocation
    }

    /// Returns only the checked body.
    pub fn body(&self) -> &[T] {
        &self.allocation[self.body.clone()]
    }

    /// Returns the exact body range used for a future protected device subview.
    pub fn body_range(&self) -> Range<usize> {
        self.body.clone()
    }
}

/// Prepared guarded inputs and independent expected C allocation.
#[derive(Clone, Debug)]
pub struct PreparedGeneralGemmHardwareCaseV1 {
    case: GeneralGemmHardwareCaseV1,
    plan: GeneralGemmPlanV1,
    a: GuardedStorageV1<u16>,
    b: GuardedStorageV1<u16>,
    c_initial: GuardedStorageV1<f32>,
    c_expected: GuardedStorageV1<f32>,
}

impl PreparedGeneralGemmHardwareCaseV1 {
    /// Returns the exact case.
    pub const fn case(&self) -> GeneralGemmHardwareCaseV1 {
        self.case
    }

    /// Returns the checked independent host plan.
    pub const fn plan(&self) -> &GeneralGemmPlanV1 {
        &self.plan
    }

    /// Returns guarded BF16 A storage.
    pub const fn a(&self) -> &GuardedStorageV1<u16> {
        &self.a
    }

    /// Returns guarded BF16 B storage.
    pub const fn b(&self) -> &GuardedStorageV1<u16> {
        &self.b
    }

    /// Returns guarded initial FP32 C storage.
    pub const fn c_initial(&self) -> &GuardedStorageV1<f32> {
        &self.c_initial
    }

    /// Returns guarded bit-exact expected FP32 C storage.
    pub const fn c_expected(&self) -> &GuardedStorageV1<f32> {
        &self.c_expected
    }
}

/// Independent preparation failed before any GPU authority existed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmHardwarePreparationErrorV1 {
    /// The checked host plan rejected the case.
    Plan(GeneralPlanErrorV1),
    /// The independent tiled CPU oracle rejected exact storage.
    Oracle(GeneralReferenceErrorV1),
}

impl fmt::Display for GeneralGemmHardwarePreparationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM hardware preparation failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmHardwarePreparationErrorV1 {}

/// Builds guarded deterministic inputs and the bit-exact independent oracle.
pub fn prepare_general_gemm_hardware_case_v1(
    case: GeneralGemmHardwareCaseV1,
) -> Result<PreparedGeneralGemmHardwareCaseV1, GeneralGemmHardwarePreparationErrorV1> {
    let [m, n, k] = case.dimensions;
    let [lda, ldb, ldc] = case.strides;
    let target = admit_target_v1(
        AmdTargetId::parse(GENERAL_GEMM_DEVICE_TARGET_V1)
            .expect("fixed general GEMM target is canonical"),
    )
    .expect("fixed general GEMM target is admitted");
    let plan = plan_general_gemm_v1(
        target,
        GeneralGemmRequestV1::new(
            m,
            n,
            k,
            lda,
            ldb,
            ldc,
            f32::from_bits(case.alpha_bits),
            f32::from_bits(case.beta_bits),
        ),
        GeneralLaunchLimitsV1::representable(),
    )
    .map_err(GeneralGemmHardwarePreparationErrorV1::Plan)?;
    let [a_len, b_len, c_len] = plan.storage().elements();
    let mut a_body = vec![A_BODY_POISON; a_len];
    let mut b_body = vec![B_BODY_POISON; b_len];
    let mut c_body = vec![C_BODY_POISON; c_len];

    for row in 0..m as usize {
        for depth in 0..k as usize {
            a_body[row * lda as usize + depth] = bf16_bits(pattern(row, depth, 3, 5, 7));
        }
    }
    for depth in 0..k as usize {
        for column in 0..n as usize {
            b_body[depth * ldb as usize + column] = bf16_bits(pattern(depth, column, 2, 3, 5));
        }
    }
    for row in 0..m as usize {
        for column in 0..n as usize {
            c_body[row * ldc as usize + column] = pattern(row, column, 7, 11, 13);
        }
    }

    let result = execute_general_reference_v1(&plan, &a_body, &b_body, &c_body)
        .map_err(GeneralGemmHardwarePreparationErrorV1::Oracle)?;
    let mut expected_body = c_body.clone();
    for row in 0..m as usize {
        for column in 0..n as usize {
            expected_body[row * ldc as usize + column] = result.output()[row * n as usize + column];
        }
    }

    Ok(PreparedGeneralGemmHardwareCaseV1 {
        case,
        plan,
        a: guarded(a_body, A_LEFT_GUARD, A_RIGHT_GUARD),
        b: guarded(b_body, B_LEFT_GUARD, B_RIGHT_GUARD),
        c_initial: guarded(c_body, C_LEFT_GUARD, C_RIGHT_GUARD),
        c_expected: guarded(expected_body, C_LEFT_GUARD, C_RIGHT_GUARD),
    })
}

/// A caller-reported allocation differed from the independent expectation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralGemmOracleComparisonErrorV1 {
    role: &'static str,
    index: usize,
}

impl GeneralGemmOracleComparisonErrorV1 {
    /// Returns the mismatched allocation role.
    pub const fn role(&self) -> &'static str {
        self.role
    }

    /// Returns the first mismatched element index.
    pub const fn index(&self) -> usize {
        self.index
    }
}

impl fmt::Display for GeneralGemmOracleComparisonErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} differs at element {}", self.role, self.index)
    }
}

impl std::error::Error for GeneralGemmOracleComparisonErrorV1 {}

/// Non-authoritative successful bitwise comparison against one prepared case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmOracleComparisonV1 {
    case: GeneralGemmHardwareCaseV1,
    compared_a_elements: usize,
    compared_b_elements: usize,
    compared_c_elements: usize,
}

impl GeneralGemmOracleComparisonV1 {
    /// Returns the exact compared case.
    pub const fn case(self) -> GeneralGemmHardwareCaseV1 {
        self.case
    }

    /// Returns complete compared allocation lengths `[A, B, C]`, including guards.
    pub const fn compared_elements(self) -> [usize; 3] {
        [
            self.compared_a_elements,
            self.compared_b_elements,
            self.compared_c_elements,
        ]
    }

    /// Oracle comparison alone never grants protected-execution evidence.
    pub const fn grants_protected_execution_evidence(self) -> bool {
        false
    }
}

/// Compares caller-reported complete allocations, including input immutability,
/// padding, and adjacent guards. This function authenticates no observer.
pub fn compare_general_gemm_hardware_observation_v1(
    prepared: &PreparedGeneralGemmHardwareCaseV1,
    observed_a: &[u16],
    observed_b: &[u16],
    observed_c: &[f32],
) -> Result<GeneralGemmOracleComparisonV1, GeneralGemmOracleComparisonErrorV1> {
    compare_u16("A allocation", observed_a, prepared.a.allocation())?;
    compare_u16("B allocation", observed_b, prepared.b.allocation())?;
    compare_f32_bits("C allocation", observed_c, prepared.c_expected.allocation())?;
    Ok(GeneralGemmOracleComparisonV1 {
        case: prepared.case,
        compared_a_elements: observed_a.len(),
        compared_b_elements: observed_b.len(),
        compared_c_elements: observed_c.len(),
    })
}

/// Exact future launch geometry and resource contract. This record is inert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmProtectedLaunchContractV1 {
    grid: [u32; 3],
    workgroup: [u32; 3],
    lds_bytes: u32,
}

impl GeneralGemmProtectedLaunchContractV1 {
    /// Derives the exact launch contract from a checked prepared case.
    pub fn from_prepared(prepared: &PreparedGeneralGemmHardwareCaseV1) -> Self {
        Self {
            grid: prepared.plan.aql_grid_work_items(),
            workgroup: GENERAL_GEMM_WORKGROUP_V1,
            lds_bytes: GENERAL_GEMM_LDS_BYTES_V1,
        }
    }

    /// Returns the 2D-tiled AQL grid in work-items.
    pub const fn grid(self) -> [u32; 3] {
        self.grid
    }

    /// Returns the required wave64 workgroup.
    pub const fn workgroup(self) -> [u32; 3] {
        self.workgroup
    }

    /// Returns the exact single-buffered LDS bytes.
    pub const fn lds_bytes(self) -> u32 {
        self.lds_bytes
    }
}

/// Linear protected authority required by the future real hardware harness.
///
/// Fields are private and there is no constructor. In particular, raw artifact
/// bytes, paths, device handles, and generic runtime adapters cannot mint it.
#[derive(Debug)]
pub struct GeneralGemmProtectedHardwareAuthorityV1 {
    compilation_binding: GeneralGemmCompilationBindingIdentityV1,
    schedule: GeneralGemmScheduleIdentityV1,
    schedule_proof: GeneralGemmEvidenceIdentityV1,
    machine_refinement: [u8; 32],
    final_proof_admission: [u8; 32],
    artifact: GeneralGemmArtifactBindingIdentityV1,
    runtime_abi: GeneralGemmRuntimeAbiIdentityV1,
    publication: [u8; 32],
    application: [u8; 32],
    observed_device: [u8; 32],
    reviewed_runtime: [u8; 32],
}

impl GeneralGemmProtectedHardwareAuthorityV1 {
    /// Returns the exact proof-to-compiler binding.
    pub const fn compilation_binding_identity(&self) -> GeneralGemmCompilationBindingIdentityV1 {
        self.compilation_binding
    }

    /// Returns the independently qualified schedule identity.
    pub const fn schedule_identity(&self) -> GeneralGemmScheduleIdentityV1 {
        self.schedule
    }

    /// Returns the verifier-owned schedule-proof identity.
    pub const fn schedule_proof_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.schedule_proof
    }

    /// Returns the exact post-artifact machine-refinement evidence identity.
    pub const fn machine_refinement_identity(&self) -> &[u8; 32] {
        &self.machine_refinement
    }

    /// Returns the verifier-owned final admission identity.
    pub const fn final_proof_admission_identity(&self) -> &[u8; 32] {
        &self.final_proof_admission
    }

    /// Returns the exact source-bound artifact identity.
    pub const fn artifact_identity(&self) -> GeneralGemmArtifactBindingIdentityV1 {
        self.artifact
    }

    /// Returns the exact dynamic runtime ABI identity.
    pub const fn runtime_abi_identity(&self) -> GeneralGemmRuntimeAbiIdentityV1 {
        self.runtime_abi
    }

    /// Returns publication, application, device, and runtime identities.
    pub const fn protected_runtime_identities(&self) -> [[u8; 32]; 4] {
        [
            self.publication,
            self.application,
            self.observed_device,
            self.reviewed_runtime,
        ]
    }
}

/// Linear eleven-slot arguments required by the future protected adapter.
///
/// The lifetime markers preserve shared A/B and unique C ownership. There is
/// no public constructor until generated device views and the checked runtime
/// ABI are joined by the protected host layer.
#[derive(Debug)]
pub struct GeneralGemmProtectedArgumentsV1<'buffers> {
    snapshot: GeneralGemmRuntimeAbiSnapshotV1,
    launch: GeneralGemmProtectedLaunchContractV1,
    _a: PhantomData<&'buffers [u16]>,
    _b: PhantomData<&'buffers [u16]>,
    _c: PhantomData<&'buffers mut [f32]>,
}

impl GeneralGemmProtectedArgumentsV1<'_> {
    /// Returns exact lengths, dimensions, strides, and coefficient bits.
    pub const fn runtime_abi(&self) -> GeneralGemmRuntimeAbiSnapshotV1 {
        self.snapshot
    }

    /// Returns exact checked geometry and LDS resources.
    pub const fn launch_contract(&self) -> GeneralGemmProtectedLaunchContractV1 {
        self.launch
    }
}

/// No protected hardware authority can be constructed at this checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingGeneralGemmProtectedAuthorityV1;

impl fmt::Display for MissingGeneralGemmProtectedAuthorityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "source-bound general GEMM artifact, final proof admission, and protected runtime authority are required",
        )
    }
}

impl std::error::Error for MissingGeneralGemmProtectedAuthorityV1 {}

/// Fails closed until the real compiler, finalizer, and runtime joins exist.
pub const fn fail_closed_without_general_gemm_protected_authority_v1()
-> Result<(), MissingGeneralGemmProtectedAuthorityV1> {
    Err(MissingGeneralGemmProtectedAuthorityV1)
}

fn guarded<T: Copy>(body: Vec<T>, left: T, right: T) -> GuardedStorageV1<T> {
    let body_range = GENERAL_GEMM_GUARD_ELEMENTS_V1..GENERAL_GEMM_GUARD_ELEMENTS_V1 + body.len();
    let mut allocation = vec![left; GENERAL_GEMM_GUARD_ELEMENTS_V1];
    allocation.extend_from_slice(&body);
    allocation.resize(body_range.end + GENERAL_GEMM_GUARD_ELEMENTS_V1, right);
    GuardedStorageV1 {
        allocation,
        body: body_range,
    }
}

fn pattern(
    row: usize,
    column: usize,
    row_factor: usize,
    column_factor: usize,
    modulus: usize,
) -> f32 {
    ((row * row_factor + column * column_factor) % modulus) as f32 - (modulus / 2) as f32
}

fn bf16_bits(value: f32) -> u16 {
    (value.to_bits() >> 16) as u16
}

fn compare_u16(
    role: &'static str,
    actual: &[u16],
    expected: &[u16],
) -> Result<(), GeneralGemmOracleComparisonErrorV1> {
    let mismatch = actual.len().min(expected.len());
    if actual.len() != expected.len() {
        return Err(GeneralGemmOracleComparisonErrorV1 {
            role,
            index: mismatch,
        });
    }
    actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
        .map_or(Ok(()), |index| {
            Err(GeneralGemmOracleComparisonErrorV1 { role, index })
        })
}

fn compare_f32_bits(
    role: &'static str,
    actual: &[f32],
    expected: &[f32],
) -> Result<(), GeneralGemmOracleComparisonErrorV1> {
    let mismatch = actual.len().min(expected.len());
    if actual.len() != expected.len() {
        return Err(GeneralGemmOracleComparisonErrorV1 {
            role,
            index: mismatch,
        });
    }
    actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual.to_bits() != expected.to_bits())
        .map_or(Ok(()), |index| {
            Err(GeneralGemmOracleComparisonErrorV1 { role, index })
        })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn matrix_covers_both_schedules_and_every_required_axis() {
        assert_eq!(GENERAL_GEMM_HARDWARE_CASES_V1.len(), 14);
        let names: BTreeSet<_> = GENERAL_GEMM_HARDWARE_CASES_V1
            .iter()
            .map(|case| case.name())
            .collect();
        assert_eq!(names.len(), 14);
        for schedule in [REFERENCE, VECTOR_A] {
            let cases: Vec<_> = GENERAL_GEMM_HARDWARE_CASES_V1
                .iter()
                .filter(|case| case.schedule() == schedule)
                .collect();
            assert_eq!(cases.len(), 7);
            assert!(cases.iter().any(|case| case.name().contains("packed")));
            assert!(
                cases
                    .iter()
                    .any(|case| case.name().contains("strided-all-tails"))
            );
            assert!(
                cases
                    .iter()
                    .any(|case| case.name().contains("multi-wg-multi-phase"))
            );
            assert!(cases.iter().any(|case| case.name().contains("m-tail")));
            assert!(cases.iter().any(|case| case.name().contains("n-tail")));
            assert!(cases.iter().any(|case| case.name().contains("k-tail")));
            assert!(cases.iter().any(|case| case.name().contains("zero-k")));
        }
        assert_ne!(REFERENCE.identity(), VECTOR_A.identity());
    }

    #[test]
    fn every_case_builds_guarded_bit_exact_oracle_storage() {
        for case in GENERAL_GEMM_HARDWARE_CASES_V1 {
            let prepared = prepare_general_gemm_hardware_case_v1(case).unwrap();
            assert_eq!(
                prepared.plan().workgroup_dimensions(),
                GENERAL_GEMM_WORKGROUP_V1
            );
            assert_eq!(prepared.plan().lds_bytes(), GENERAL_GEMM_LDS_BYTES_V1);
            assert_eq!(
                prepared.a().allocation().len(),
                prepared.a().body().len() + 2 * GENERAL_GEMM_GUARD_ELEMENTS_V1
            );
            assert_eq!(
                prepared.b().allocation().len(),
                prepared.b().body().len() + 2 * GENERAL_GEMM_GUARD_ELEMENTS_V1
            );
            assert_eq!(
                prepared.c_initial().allocation().len(),
                prepared.c_initial().body().len() + 2 * GENERAL_GEMM_GUARD_ELEMENTS_V1
            );
            let comparison = compare_general_gemm_hardware_observation_v1(
                &prepared,
                prepared.a().allocation(),
                prepared.b().allocation(),
                prepared.c_expected().allocation(),
            )
            .unwrap();
            assert_eq!(comparison.case(), case);
            assert!(!comparison.grants_protected_execution_evidence());

            let launch = GeneralGemmProtectedLaunchContractV1::from_prepared(&prepared);
            assert_eq!(launch.grid(), prepared.plan().aql_grid_work_items());
            assert_eq!(launch.workgroup(), GENERAL_GEMM_WORKGROUP_V1);
            assert_eq!(launch.lds_bytes(), GENERAL_GEMM_LDS_BYTES_V1);
        }
    }

    #[test]
    fn guards_padding_inputs_and_logical_outputs_are_all_observed() {
        let prepared =
            prepare_general_gemm_hardware_case_v1(GENERAL_GEMM_HARDWARE_CASES_V1[1]).unwrap();
        let mut a = prepared.a().allocation().to_vec();
        let mut b = prepared.b().allocation().to_vec();
        let mut c = prepared.c_expected().allocation().to_vec();

        for (role, index) in [
            ("A allocation", 0),
            ("A allocation", prepared.a().body_range().start + 18),
        ] {
            a[index] ^= 1;
            let error =
                compare_general_gemm_hardware_observation_v1(&prepared, &a, &b, &c).unwrap_err();
            assert_eq!((error.role(), error.index()), (role, index));
            a[index] ^= 1;
        }
        let b_index = prepared.b().body_range().start + prepared.case().strides()[1] as usize - 1;
        b[b_index] ^= 1;
        assert_eq!(
            compare_general_gemm_hardware_observation_v1(&prepared, &a, &b, &c)
                .unwrap_err()
                .role(),
            "B allocation"
        );
        b[b_index] ^= 1;

        let c_index = prepared.c_expected().body_range().start;
        c[c_index] = f32::from_bits(c[c_index].to_bits() ^ 1);
        assert_eq!(
            compare_general_gemm_hardware_observation_v1(&prepared, &a, &b, &c)
                .unwrap_err()
                .role(),
            "C allocation"
        );
        c[c_index] = f32::from_bits(c[c_index].to_bits() ^ 1);

        for index in [
            0,
            prepared.c_expected().body_range().start + prepared.case().strides()[2] as usize - 1,
            prepared.c_expected().allocation().len() - 1,
        ] {
            c[index] = f32::from_bits(c[index].to_bits() ^ 1);
            let error =
                compare_general_gemm_hardware_observation_v1(&prepared, &a, &b, &c).unwrap_err();
            assert_eq!((error.role(), error.index()), ("C allocation", index));
            c[index] = f32::from_bits(c[index].to_bits() ^ 1);
        }
    }

    #[test]
    fn zero_k_uses_beta_and_no_operand_storage_for_both_schedules() {
        for case in GENERAL_GEMM_HARDWARE_CASES_V1
            .iter()
            .filter(|case| case.name().contains("zero-k"))
        {
            let prepared = prepare_general_gemm_hardware_case_v1(*case).unwrap();
            assert!(prepared.a().body().is_empty());
            assert!(prepared.b().body().is_empty());
            assert_eq!(prepared.plan().reduction_phases(), 0);
            let ldc = case.strides()[2] as usize;
            let [m, n, _] = case.dimensions();
            for row in 0..m as usize {
                for column in 0..n as usize {
                    let index = row * ldc + column;
                    assert_eq!(
                        prepared.c_expected().body()[index],
                        0.5 * prepared.c_initial().body()[index]
                    );
                }
            }
        }
    }

    #[test]
    fn authority_is_unavailable_and_no_execution_claim_is_made() {
        assert!(!std::hint::black_box(
            GENERAL_GEMM_PROTECTED_EXECUTION_AVAILABLE_V1
        ));
        assert_eq!(
            fail_closed_without_general_gemm_protected_authority_v1(),
            Err(MissingGeneralGemmProtectedAuthorityV1)
        );
    }
}

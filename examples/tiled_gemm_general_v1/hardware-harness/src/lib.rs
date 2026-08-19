#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Guarded case preparation and fail-closed protected-execution contracts for
//! the issue #138 general tiled GEMM.
//!
//! This package cannot yet perform device execution because the authority type
//! intentionally has no constructor. The generated user kernel remains safe
//! Rust; only the private reviewed host adapter contains documented native HSA
//! calls, behind the unavailable final authority.

use core::{fmt, ops::Range};

use fe2o3_amd_target::AmdTargetId;
use fe2o3_general_gemm_compiler::{
    GENERAL_GEMM_DEVICE_TARGET_V1, GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1,
    GENERAL_GEMM_KERNEL_SYMBOL_V1, GENERAL_GEMM_TOTAL_KERNARG_BYTES_V1,
    GeneralGemmArtifactBindingIdentityV1, GeneralGemmCompilationBindingIdentityV1,
    GeneralGemmRuntimeAbiIdentityV1, GeneralGemmRuntimeAbiSnapshotV1,
    GeneralGemmScheduleIdentityV1, GeneralGemmScheduleV1,
};
use fe2o3_host::{
    HsaDispatchObservationV1, HsaExecutableObjectIdentityV1,
    HsaImplicitKernargInitializationObservationV1, HsaKernelObjectIdentityV1,
    HsaKernelResolutionObservationV1, HsaLaunchGeometryV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
use fe2o3_hsa_runtime::{
    HsaRuntimeAdapterError, ReviewedHsaExecutableV1, ReviewedHsaHardwareTestBufferV1,
    ReviewedHsaKernelV1, ReviewedHsaRuntimeAdapterV1,
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
/// Number of physical kernarg components after lowering three slice pairs.
pub const GENERAL_GEMM_PHYSICAL_ARGUMENT_COUNT_V1: usize = 14;
/// Fixed AMDHSA code-object V6 implicit kernarg suffix.
pub const GENERAL_GEMM_IMPLICIT_KERNARG_BYTES_V1: usize = 256;
/// Stack alignment used for the complete generated kernarg image.
pub const GENERAL_GEMM_KERNARG_STORAGE_ALIGNMENT_V1: usize = 16;
/// Exact HSA kernarg-segment alignment in the compiler-generated descriptor.
pub const GENERAL_GEMM_KERNARG_SEGMENT_ALIGNMENT_V1: u64 = 8;
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

/// Physical role of one compiler-generated kernarg component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmPhysicalArgumentKindV1 {
    /// Global device pointer from a logical slice.
    SlicePointer,
    /// `u64` element count from a logical slice.
    SliceLength,
    /// By-value `u32` scalar.
    U32,
    /// By-value FP32 bit pattern.
    F32,
}

/// One exact component in the generated 11-logical/14-physical ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmPhysicalArgumentLayoutV1 {
    logical_index: u8,
    kind: GeneralGemmPhysicalArgumentKindV1,
    offset: u8,
    size: u8,
}

impl GeneralGemmPhysicalArgumentLayoutV1 {
    const fn new(
        logical_index: u8,
        kind: GeneralGemmPhysicalArgumentKindV1,
        offset: u8,
        size: u8,
    ) -> Self {
        Self {
            logical_index,
            kind,
            offset,
            size,
        }
    }

    /// Returns the source-level argument position in `0..11`.
    pub const fn logical_index(self) -> u8 {
        self.logical_index
    }

    /// Returns the lowered physical component role.
    pub const fn kind(self) -> GeneralGemmPhysicalArgumentKindV1 {
        self.kind
    }

    /// Returns `[byte offset, byte size]` in the explicit kernarg prefix.
    pub const fn byte_layout(self) -> [u8; 2] {
        [self.offset, self.size]
    }
}

const SLICE_POINTER: GeneralGemmPhysicalArgumentKindV1 =
    GeneralGemmPhysicalArgumentKindV1::SlicePointer;
const SLICE_LENGTH: GeneralGemmPhysicalArgumentKindV1 =
    GeneralGemmPhysicalArgumentKindV1::SliceLength;
const U32_ARGUMENT: GeneralGemmPhysicalArgumentKindV1 = GeneralGemmPhysicalArgumentKindV1::U32;
const F32_ARGUMENT: GeneralGemmPhysicalArgumentKindV1 = GeneralGemmPhysicalArgumentKindV1::F32;

/// Canonical physical ABI emitted by the generated general-GEMM adapter.
pub const GENERAL_GEMM_PHYSICAL_ARGUMENT_LAYOUT_V1: [GeneralGemmPhysicalArgumentLayoutV1;
    GENERAL_GEMM_PHYSICAL_ARGUMENT_COUNT_V1] = [
    GeneralGemmPhysicalArgumentLayoutV1::new(0, SLICE_POINTER, 0, 8),
    GeneralGemmPhysicalArgumentLayoutV1::new(0, SLICE_LENGTH, 8, 8),
    GeneralGemmPhysicalArgumentLayoutV1::new(1, SLICE_POINTER, 16, 8),
    GeneralGemmPhysicalArgumentLayoutV1::new(1, SLICE_LENGTH, 24, 8),
    GeneralGemmPhysicalArgumentLayoutV1::new(2, SLICE_POINTER, 32, 8),
    GeneralGemmPhysicalArgumentLayoutV1::new(2, SLICE_LENGTH, 40, 8),
    GeneralGemmPhysicalArgumentLayoutV1::new(3, U32_ARGUMENT, 48, 4),
    GeneralGemmPhysicalArgumentLayoutV1::new(4, U32_ARGUMENT, 52, 4),
    GeneralGemmPhysicalArgumentLayoutV1::new(5, U32_ARGUMENT, 56, 4),
    GeneralGemmPhysicalArgumentLayoutV1::new(6, U32_ARGUMENT, 60, 4),
    GeneralGemmPhysicalArgumentLayoutV1::new(7, U32_ARGUMENT, 64, 4),
    GeneralGemmPhysicalArgumentLayoutV1::new(8, U32_ARGUMENT, 68, 4),
    GeneralGemmPhysicalArgumentLayoutV1::new(9, F32_ARGUMENT, 72, 4),
    GeneralGemmPhysicalArgumentLayoutV1::new(10, F32_ARGUMENT, 76, 4),
];

/// Linear protected authority required by the future real hardware harness.
///
/// Fields are private and there is no constructor. In particular, raw artifact
/// bytes, paths, device handles, and generic runtime adapters cannot mint it.
#[derive(Debug)]
pub struct GeneralGemmProtectedHardwareAuthorityV1 {
    compilation_binding: GeneralGemmCompilationBindingIdentityV1,
    schedule: GeneralGemmScheduleIdentityV1,
    schedule_proof: GeneralGemmEvidenceIdentityV1,
    proof_and_numerical: [u8; 32],
    machine_inspection: [u8; 32],
    rustc_final_join: [u8; 32],
    artifact: GeneralGemmArtifactBindingIdentityV1,
    runtime_abi: GeneralGemmRuntimeAbiIdentityV1,
    runtime_abi_snapshot: GeneralGemmRuntimeAbiSnapshotV1,
    publication: [u8; 32],
    application: [u8; 32],
    observed_device: [u8; 32],
    reviewed_runtime: [u8; 32],
    hsa_executable: HsaExecutableObjectIdentityV1,
    hsa_kernel: HsaKernelObjectIdentityV1,
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

    /// Returns the verifier-owned schedule/numerical evidence identity.
    pub const fn proof_and_numerical_identity(&self) -> &[u8; 32] {
        &self.proof_and_numerical
    }

    /// Returns finalizer machine inspection and rustc three-owner join identities.
    pub const fn final_authority_identities(&self) -> [[u8; 32]; 2] {
        [self.machine_inspection, self.rustc_final_join]
    }

    /// Returns the exact source-bound artifact identity.
    pub const fn artifact_identity(&self) -> GeneralGemmArtifactBindingIdentityV1 {
        self.artifact
    }

    /// Returns the exact dynamic runtime ABI identity.
    pub const fn runtime_abi_identity(&self) -> GeneralGemmRuntimeAbiIdentityV1 {
        self.runtime_abi
    }

    /// Returns exact launch-time values joined to the runtime ABI identity.
    pub const fn runtime_abi_snapshot(&self) -> GeneralGemmRuntimeAbiSnapshotV1 {
        self.runtime_abi_snapshot
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

    /// Returns the exact loaded executable identity.
    pub const fn hsa_executable_identity(&self) -> HsaExecutableObjectIdentityV1 {
        self.hsa_executable
    }

    /// Returns the exact resolved-kernel identity.
    pub const fn hsa_kernel_identity(&self) -> HsaKernelObjectIdentityV1 {
        self.hsa_kernel
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneralGemmPhysicalArgumentsV1 {
    a_address: u64,
    b_address: u64,
    c_address: u64,
}

/// Linear eleven-slot arguments required by the protected adapter.
///
/// Construction requires the unforgeable final authority. Shared A/B and
/// unique C borrows retain all three HSA allocations through synchronous
/// completion; no pointer or raw kernarg accessor is exposed.
pub struct GeneralGemmProtectedArgumentsV1<'buffers> {
    snapshot: GeneralGemmRuntimeAbiSnapshotV1,
    runtime_abi_identity: GeneralGemmRuntimeAbiIdentityV1,
    schedule_identity: GeneralGemmScheduleIdentityV1,
    launch: GeneralGemmProtectedLaunchContractV1,
    physical: GeneralGemmPhysicalArgumentsV1,
    _a: &'buffers ReviewedHsaHardwareTestBufferV1,
    _b: &'buffers ReviewedHsaHardwareTestBufferV1,
    _c: &'buffers mut ReviewedHsaHardwareTestBufferV1,
}

impl<'buffers> GeneralGemmProtectedArgumentsV1<'buffers> {
    /// Checks exact guarded buffer extents and derives all 14 physical values.
    pub fn checked_hardware_buffers(
        authority: &GeneralGemmProtectedHardwareAuthorityV1,
        prepared: &PreparedGeneralGemmHardwareCaseV1,
        a: &'buffers ReviewedHsaHardwareTestBufferV1,
        b: &'buffers ReviewedHsaHardwareTestBufferV1,
        c: &'buffers mut ReviewedHsaHardwareTestBufferV1,
    ) -> Result<Self, GeneralGemmProtectedLaunchErrorV1> {
        let snapshot = runtime_snapshot(prepared)?;
        if snapshot != authority.runtime_abi_snapshot
            || prepared.case.schedule.identity() != authority.schedule
        {
            return Err(GeneralGemmProtectedLaunchErrorV1::AuthoritySubstitution);
        }
        let a_bytes = checked_element_bytes(prepared.a.allocation().len(), 2, "A")?;
        let b_bytes = checked_element_bytes(prepared.b.allocation().len(), 2, "B")?;
        let c_bytes = checked_element_bytes(prepared.c_initial.allocation().len(), 4, "C")?;
        for (actual, expected, role) in [
            (a.byte_len(), a_bytes, "A"),
            (b.byte_len(), b_bytes, "B"),
            (c.byte_len(), c_bytes, "C"),
        ] {
            if actual != expected {
                return Err(GeneralGemmProtectedLaunchErrorV1::BufferExtent { role });
            }
        }
        let physical = GeneralGemmPhysicalArgumentsV1 {
            a_address: a.device_address(checked_element_bytes(prepared.a.body.start, 2, "A")?)?,
            b_address: b.device_address(checked_element_bytes(prepared.b.body.start, 2, "B")?)?,
            c_address: c.device_address(checked_element_bytes(
                prepared.c_initial.body.start,
                4,
                "C",
            )?)?,
        };
        Ok(Self {
            snapshot,
            runtime_abi_identity: authority.runtime_abi,
            schedule_identity: authority.schedule,
            launch: GeneralGemmProtectedLaunchContractV1::from_prepared(prepared),
            physical,
            _a: a,
            _b: b,
            _c: c,
        })
    }

    /// Returns exact lengths, dimensions, strides, and coefficient bits.
    pub const fn runtime_abi(&self) -> GeneralGemmRuntimeAbiSnapshotV1 {
        self.snapshot
    }

    /// Returns exact checked geometry and LDS resources.
    pub const fn launch_contract(&self) -> GeneralGemmProtectedLaunchContractV1 {
        self.launch
    }
}

/// Failure before or during the one-shot protected HSA dispatch.
#[derive(Debug)]
#[non_exhaustive]
pub enum GeneralGemmProtectedLaunchErrorV1 {
    /// A runtime value conflicted with the consumed final authority.
    AuthoritySubstitution,
    /// A guarded HSA allocation did not have the exact host allocation extent.
    BufferExtent {
        /// Matrix whose allocation extent differed.
        role: &'static str,
    },
    /// An element-to-byte conversion overflowed before any native operation.
    BufferExtentOverflow {
        /// Matrix whose byte conversion overflowed.
        role: &'static str,
    },
    /// The reviewed HSA adapter rejected an allocation, kernarg, or dispatch.
    Hsa(HsaRuntimeAdapterError),
    /// A loaded-object or resolved-kernel observation conflicted with authority.
    ResolutionSubstitution(&'static str),
    /// The reviewed implicit-kernarg observation conflicted with the exact call.
    ImplicitObservationSubstitution(&'static str),
    /// Implicit-kernarg preparation or dispatch changed the generated prefix.
    ExplicitKernargMutation,
    /// The synchronous dispatch observation conflicted with the exact call.
    DispatchObservationSubstitution(&'static str),
}

impl fmt::Display for GeneralGemmProtectedLaunchErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthoritySubstitution => {
                formatter.write_str("general GEMM runtime values substituted final authority")
            }
            Self::BufferExtent { role } => {
                write!(
                    formatter,
                    "general GEMM {role} HSA allocation has the wrong extent"
                )
            }
            Self::BufferExtentOverflow { role } => {
                write!(
                    formatter,
                    "general GEMM {role} allocation byte extent overflowed"
                )
            }
            Self::Hsa(error) => write!(formatter, "reviewed HSA operation failed: {error}"),
            Self::ResolutionSubstitution(field) => {
                write!(formatter, "general GEMM HSA resolution substituted {field}")
            }
            Self::ImplicitObservationSubstitution(field) => {
                write!(
                    formatter,
                    "general GEMM implicit kernarg substituted {field}"
                )
            }
            Self::ExplicitKernargMutation => {
                formatter.write_str("reviewed HSA operation mutated explicit general GEMM kernargs")
            }
            Self::DispatchObservationSubstitution(field) => {
                write!(formatter, "general GEMM HSA dispatch substituted {field}")
            }
        }
    }
}

impl std::error::Error for GeneralGemmProtectedLaunchErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hsa(error) => Some(error),
            _ => None,
        }
    }
}

impl From<HsaRuntimeAdapterError> for GeneralGemmProtectedLaunchErrorV1 {
    fn from(error: HsaRuntimeAdapterError) -> Self {
        Self::Hsa(error)
    }
}

/// Typed completion of one authority-bound synchronous HSA dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmProtectedDispatchCompletionV1 {
    compilation_binding: GeneralGemmCompilationBindingIdentityV1,
    artifact: GeneralGemmArtifactBindingIdentityV1,
    schedule: GeneralGemmScheduleIdentityV1,
    dispatch: [u8; 16],
}

impl GeneralGemmProtectedDispatchCompletionV1 {
    /// Returns the compilation unit bound by the consumed authority.
    pub const fn compilation_binding_identity(&self) -> GeneralGemmCompilationBindingIdentityV1 {
        self.compilation_binding
    }

    /// Returns the finalized artifact binding bound by the consumed authority.
    pub const fn artifact_identity(&self) -> GeneralGemmArtifactBindingIdentityV1 {
        self.artifact
    }

    /// Returns the independently qualified schedule identity.
    pub const fn schedule_identity(&self) -> GeneralGemmScheduleIdentityV1 {
        self.schedule
    }

    /// Returns the reviewed synchronous HSA dispatch identity.
    pub const fn dispatch_identity(&self) -> [u8; 16] {
        self.dispatch
    }

    /// Dispatch completion alone is not bitwise CPU-oracle evidence.
    pub const fn grants_protected_execution_evidence(&self) -> bool {
        false
    }
}

#[repr(C, align(16))]
struct GeneralGemmAlignedKernargV1([u8; GENERAL_GEMM_TOTAL_KERNARG_BYTES_V1 as usize]);

/// Packs and synchronously dispatches the exact generated general-GEMM ABI.
///
/// This is the only launch surface in this package. It consumes the opaque
/// final authority, accepts only reviewed HSA objects, and exposes neither
/// device addresses nor the generated kernarg bytes. The authority currently
/// has no constructor; rustc-codegen will own that final construction join.
pub fn launch_general_gemm_protected_v1(
    authority: GeneralGemmProtectedHardwareAuthorityV1,
    arguments: GeneralGemmProtectedArgumentsV1<'_>,
    runtime: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
) -> Result<GeneralGemmProtectedDispatchCompletionV1, GeneralGemmProtectedLaunchErrorV1> {
    if arguments.runtime_abi_identity != authority.runtime_abi
        || arguments.snapshot != authority.runtime_abi_snapshot
        || arguments.schedule_identity != authority.schedule
        || arguments.launch.workgroup != GENERAL_GEMM_WORKGROUP_V1
        || arguments.launch.lds_bytes != GENERAL_GEMM_LDS_BYTES_V1
        || arguments.launch.grid.contains(&0)
    {
        return Err(GeneralGemmProtectedLaunchErrorV1::AuthoritySubstitution);
    }
    validate_resolution(&authority, resolution)?;

    let geometry = HsaLaunchGeometryV1::new(arguments.launch.grid, arguments.launch.workgroup, 0);
    let explicit = pack_explicit_kernarg(arguments.snapshot, arguments.physical);
    let mut kernarg =
        GeneralGemmAlignedKernargV1([0; GENERAL_GEMM_TOTAL_KERNARG_BYTES_V1 as usize]);
    kernarg.0[..explicit.len()].copy_from_slice(&explicit);
    let explicit_before = explicit;

    // SAFETY: the only safe entry consumes final rustc-owned authority and
    // retains all three checked HSA allocations in `arguments` until both
    // reviewed operations return. The exact handles, geometry, and complete
    // compiler-generated COV6 kernarg are independently checked here.
    let implicit = unsafe {
        runtime.initialize_implicit_kernarg(
            executable,
            kernel,
            geometry,
            GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1 as usize,
            GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1 as usize,
            GENERAL_GEMM_IMPLICIT_KERNARG_BYTES_V1,
            &mut kernarg.0,
        )
    }?;
    if kernarg.0[..explicit_before.len()] != explicit_before {
        return Err(GeneralGemmProtectedLaunchErrorV1::ExplicitKernargMutation);
    }
    validate_implicit_observation(&authority, geometry, &implicit)?;

    // SAFETY: implicit initialization succeeded for the same retained handles,
    // geometry, and byte-exact kernarg. The reviewed adapter contract returns
    // only before submission or after every device effect has quiesced.
    let dispatch =
        unsafe { runtime.launch_and_wait(executable, kernel, geometry, &mut kernarg.0) }?;
    if kernarg.0[..explicit_before.len()] != explicit_before {
        return Err(GeneralGemmProtectedLaunchErrorV1::ExplicitKernargMutation);
    }
    validate_dispatch_observation(&authority, geometry, &dispatch)?;

    Ok(GeneralGemmProtectedDispatchCompletionV1 {
        compilation_binding: authority.compilation_binding,
        artifact: authority.artifact,
        schedule: authority.schedule,
        dispatch: dispatch.dispatch_identity(),
    })
}

fn runtime_snapshot(
    prepared: &PreparedGeneralGemmHardwareCaseV1,
) -> Result<GeneralGemmRuntimeAbiSnapshotV1, GeneralGemmProtectedLaunchErrorV1> {
    let [a_elements, b_elements, c_elements] = prepared.plan.storage().elements();
    let request = prepared.plan.request();
    Ok(GeneralGemmRuntimeAbiSnapshotV1 {
        a_elements: u64::try_from(a_elements)
            .map_err(|_| GeneralGemmProtectedLaunchErrorV1::BufferExtentOverflow { role: "A" })?,
        b_elements: u64::try_from(b_elements)
            .map_err(|_| GeneralGemmProtectedLaunchErrorV1::BufferExtentOverflow { role: "B" })?,
        c_elements: u64::try_from(c_elements)
            .map_err(|_| GeneralGemmProtectedLaunchErrorV1::BufferExtentOverflow { role: "C" })?,
        dimensions: request.dimensions(),
        strides: request.strides(),
        alpha_bits: request.alpha_bits(),
        beta_bits: request.beta_bits(),
    })
}

fn checked_element_bytes(
    elements: usize,
    element_bytes: usize,
    role: &'static str,
) -> Result<usize, GeneralGemmProtectedLaunchErrorV1> {
    elements
        .checked_mul(element_bytes)
        .ok_or(GeneralGemmProtectedLaunchErrorV1::BufferExtentOverflow { role })
}

fn pack_explicit_kernarg(
    snapshot: GeneralGemmRuntimeAbiSnapshotV1,
    physical: GeneralGemmPhysicalArgumentsV1,
) -> [u8; GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1 as usize] {
    let mut bytes = [0; GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1 as usize];
    put_u64(&mut bytes, 0, physical.a_address);
    put_u64(&mut bytes, 8, snapshot.a_elements);
    put_u64(&mut bytes, 16, physical.b_address);
    put_u64(&mut bytes, 24, snapshot.b_elements);
    put_u64(&mut bytes, 32, physical.c_address);
    put_u64(&mut bytes, 40, snapshot.c_elements);
    for (offset, value) in [48, 52, 56].into_iter().zip(snapshot.dimensions) {
        put_u32(&mut bytes, offset, value);
    }
    for (offset, value) in [60, 64, 68].into_iter().zip(snapshot.strides) {
        put_u32(&mut bytes, offset, value);
    }
    put_u32(&mut bytes, 72, snapshot.alpha_bits);
    put_u32(&mut bytes, 76, snapshot.beta_bits);
    bytes
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn validate_resolution(
    authority: &GeneralGemmProtectedHardwareAuthorityV1,
    resolution: &HsaKernelResolutionObservationV1,
) -> Result<(), GeneralGemmProtectedLaunchErrorV1> {
    for (matches, field) in [
        (
            resolution.executable_object() == authority.hsa_executable,
            "executable object",
        ),
        (
            resolution.kernel_object() == authority.hsa_kernel,
            "kernel object",
        ),
        (
            resolution.export_symbol() == GENERAL_GEMM_KERNEL_SYMBOL_V1,
            "export symbol",
        ),
        (
            resolution.kernarg_segment_size() == u64::from(GENERAL_GEMM_TOTAL_KERNARG_BYTES_V1),
            "kernarg segment size",
        ),
        (
            resolution.kernarg_segment_alignment() == GENERAL_GEMM_KERNARG_SEGMENT_ALIGNMENT_V1,
            "kernarg segment alignment",
        ),
    ] {
        if !matches {
            return Err(GeneralGemmProtectedLaunchErrorV1::ResolutionSubstitution(
                field,
            ));
        }
    }
    Ok(())
}

fn validate_implicit_observation(
    authority: &GeneralGemmProtectedHardwareAuthorityV1,
    geometry: HsaLaunchGeometryV1,
    observation: &HsaImplicitKernargInitializationObservationV1,
) -> Result<(), GeneralGemmProtectedLaunchErrorV1> {
    for (matches, field) in [
        (
            observation.executable_object() == authority.hsa_executable,
            "executable object",
        ),
        (
            observation.kernel_object() == authority.hsa_kernel,
            "kernel object",
        ),
        (observation.geometry() == geometry, "launch geometry"),
        (
            observation.explicit_byte_len() == u64::from(GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1),
            "explicit byte length",
        ),
        (
            observation.implicit_byte_offset() == u64::from(GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1),
            "implicit byte offset",
        ),
        (
            observation.implicit_byte_len() == GENERAL_GEMM_IMPLICIT_KERNARG_BYTES_V1 as u64,
            "implicit byte length",
        ),
        (observation.initialized(), "initialization completion"),
    ] {
        if !matches {
            return Err(GeneralGemmProtectedLaunchErrorV1::ImplicitObservationSubstitution(field));
        }
    }
    Ok(())
}

fn validate_dispatch_observation(
    authority: &GeneralGemmProtectedHardwareAuthorityV1,
    geometry: HsaLaunchGeometryV1,
    observation: &HsaDispatchObservationV1,
) -> Result<(), GeneralGemmProtectedLaunchErrorV1> {
    for (matches, field) in [
        (
            observation.executable_object() == authority.hsa_executable,
            "executable object",
        ),
        (
            observation.kernel_object() == authority.hsa_kernel,
            "kernel object",
        ),
        (observation.geometry() == geometry, "launch geometry"),
        (observation.completed(), "synchronous completion"),
    ] {
        if !matches {
            return Err(GeneralGemmProtectedLaunchErrorV1::DispatchObservationSubstitution(field));
        }
    }
    Ok(())
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
    fn generated_physical_layout_is_exactly_eleven_logical_and_fourteen_physical() {
        assert_eq!(GENERAL_GEMM_ARGUMENT_COUNT_V1, 11);
        assert_eq!(GENERAL_GEMM_PHYSICAL_ARGUMENT_LAYOUT_V1.len(), 14);
        let logical: BTreeSet<_> = GENERAL_GEMM_PHYSICAL_ARGUMENT_LAYOUT_V1
            .iter()
            .map(|component| component.logical_index())
            .collect();
        assert_eq!(logical, (0_u8..11).collect());

        let mut next_offset = 0_u8;
        for component in GENERAL_GEMM_PHYSICAL_ARGUMENT_LAYOUT_V1 {
            let [offset, size] = component.byte_layout();
            assert_eq!(offset, next_offset);
            next_offset = next_offset.checked_add(size).unwrap();
        }
        assert_eq!(next_offset, GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1 as u8);
        assert_eq!(
            GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1 as usize
                + GENERAL_GEMM_IMPLICIT_KERNARG_BYTES_V1,
            GENERAL_GEMM_TOTAL_KERNARG_BYTES_V1 as usize
        );
        assert_eq!(
            core::mem::size_of::<GeneralGemmAlignedKernargV1>(),
            GENERAL_GEMM_TOTAL_KERNARG_BYTES_V1 as usize
        );
        assert_eq!(
            core::mem::align_of::<GeneralGemmAlignedKernargV1>(),
            GENERAL_GEMM_KERNARG_STORAGE_ALIGNMENT_V1
        );
    }

    #[test]
    fn generated_packer_writes_all_fourteen_components_at_exact_offsets() {
        let snapshot = GeneralGemmRuntimeAbiSnapshotV1 {
            a_elements: 0x0102_0304_0506_0708,
            b_elements: 0x1112_1314_1516_1718,
            c_elements: 0x2122_2324_2526_2728,
            dimensions: [0x3132_3334, 0x4142_4344, 0x5152_5354],
            strides: [0x6162_6364, 0x7172_7374, 0x8182_8384],
            alpha_bits: 0x9192_9394,
            beta_bits: 0xa1a2_a3a4,
        };
        let physical = GeneralGemmPhysicalArgumentsV1 {
            a_address: 0xb1b2_b3b4_b5b6_b7b8,
            b_address: 0xc1c2_c3c4_c5c6_c7c8,
            c_address: 0xd1d2_d3d4_d5d6_d7d8,
        };
        let packed = pack_explicit_kernarg(snapshot, physical);
        let expected = [
            physical.a_address.to_le_bytes().as_slice(),
            snapshot.a_elements.to_le_bytes().as_slice(),
            physical.b_address.to_le_bytes().as_slice(),
            snapshot.b_elements.to_le_bytes().as_slice(),
            physical.c_address.to_le_bytes().as_slice(),
            snapshot.c_elements.to_le_bytes().as_slice(),
            snapshot.dimensions[0].to_le_bytes().as_slice(),
            snapshot.dimensions[1].to_le_bytes().as_slice(),
            snapshot.dimensions[2].to_le_bytes().as_slice(),
            snapshot.strides[0].to_le_bytes().as_slice(),
            snapshot.strides[1].to_le_bytes().as_slice(),
            snapshot.strides[2].to_le_bytes().as_slice(),
            snapshot.alpha_bits.to_le_bytes().as_slice(),
            snapshot.beta_bits.to_le_bytes().as_slice(),
        ]
        .concat();
        assert_eq!(packed.as_slice(), expected);
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

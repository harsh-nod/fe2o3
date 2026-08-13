use std::fmt;

pub const SCALAR_GEMM_TARGET_V1: &str = "gfx942:xnack-";
pub const SCALAR_GEMM_COVERAGE_PROFILE_V1: &str = "COV6";
pub const SCALAR_GEMM_ROOT_SYMBOL_V1: &str = "scalar_gemm_v1";
pub const SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1: u32 = 1;
pub const SCALAR_GEMM_WORKGROUP_THREADS_V1: u64 = 256;
pub const SCALAR_GEMM_MAX_GRID_THREADS_V1: u64 =
    (u32::MAX as u64 / SCALAR_GEMM_WORKGROUP_THREADS_V1) * SCALAR_GEMM_WORKGROUP_THREADS_V1;

const F32_BYTES: u64 = size_of::<f32>() as u64;

/// Checked dimensions and host-representable extents for scalar GEMM V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmShapeV1 {
    m: u32,
    n: u32,
    k: u32,
    a_elements: u64,
    b_elements: u64,
    c_elements: u64,
    a_len: usize,
    b_len: usize,
    c_len: usize,
    a_bytes: usize,
    b_bytes: usize,
    c_bytes: usize,
}

impl ScalarGemmShapeV1 {
    pub fn checked(m: u32, n: u32, k: u32) -> Result<Self, ScalarGemmHostAdmissionErrorV1> {
        let (a_elements, a_len, a_bytes) = checked_extent("A", m, k)?;
        let (b_elements, b_len, b_bytes) = checked_extent("B", k, n)?;
        let (c_elements, c_len, c_bytes) = checked_extent("C", m, n)?;
        Ok(Self {
            m,
            n,
            k,
            a_elements,
            b_elements,
            c_elements,
            a_len,
            b_len,
            c_len,
            a_bytes,
            b_bytes,
            c_bytes,
        })
    }

    pub const fn m(self) -> u32 {
        self.m
    }

    pub const fn n(self) -> u32 {
        self.n
    }

    pub const fn k(self) -> u32 {
        self.k
    }

    pub const fn a_len(self) -> usize {
        self.a_len
    }

    pub const fn b_len(self) -> usize {
        self.b_len
    }

    pub const fn c_len(self) -> usize {
        self.c_len
    }

    pub const fn a_bytes(self) -> usize {
        self.a_bytes
    }

    pub const fn b_bytes(self) -> usize {
        self.b_bytes
    }

    pub const fn c_bytes(self) -> usize {
        self.c_bytes
    }

    pub const fn output_elements_u64(self) -> u64 {
        self.c_elements
    }

    pub const fn rounded_grid_threads(self) -> u64 {
        if self.c_elements == 0 {
            0
        } else {
            ((self.c_elements - 1) / SCALAR_GEMM_WORKGROUP_THREADS_V1 + 1)
                * SCALAR_GEMM_WORKGROUP_THREADS_V1
        }
    }

    /// Maps one active linear invocation to its unique row-major output.
    pub fn invocation(self, p: u64) -> Option<ScalarGemmInvocationV1> {
        if p >= self.c_elements {
            return None;
        }
        // An active invocation implies m*n > 0 and therefore n > 0.
        let n = u64::from(self.n);
        let row = p / n;
        let col = p % n;
        Some(ScalarGemmInvocationV1 {
            p,
            row: row as u32,
            col: col as u32,
        })
    }

    pub fn dot_recurrence(self, p: u64) -> Option<ScalarGemmDotRecurrenceV1> {
        self.invocation(p)
            .map(|invocation| ScalarGemmDotRecurrenceV1 {
                shape: self,
                invocation,
                next_t: 0,
            })
    }
}

fn checked_extent(
    field: &'static str,
    left: u32,
    right: u32,
) -> Result<(u64, usize, usize), ScalarGemmHostAdmissionErrorV1> {
    let elements = u64::from(left)
        .checked_mul(u64::from(right))
        .ok_or(ScalarGemmHostAdmissionErrorV1::ElementCountOverflow { field })?;
    let bytes = elements
        .checked_mul(F32_BYTES)
        .ok_or(ScalarGemmHostAdmissionErrorV1::ByteCountOverflow { field })?;
    let host_elements = usize::try_from(elements)
        .map_err(|_| ScalarGemmHostAdmissionErrorV1::HostSizeOverflow { field })?;
    let host_bytes = usize::try_from(bytes)
        .map_err(|_| ScalarGemmHostAdmissionErrorV1::HostSizeOverflow { field })?;
    Ok((elements, host_elements, host_bytes))
}

/// Declared allocation-relative host region. The allocation identity and all
/// address metadata must come from separately authenticated runtime
/// provenance. Caller construction of this value cannot prove physical
/// identity, disjointness, liveness, or address-space membership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmBufferRegionV1 {
    declared_allocation_id: u64,
    address_space: u32,
    base_address: usize,
    allocation_byte_len: usize,
    byte_offset: usize,
    byte_len: usize,
}

impl ScalarGemmBufferRegionV1 {
    pub const fn new(
        declared_allocation_id: u64,
        address_space: u32,
        base_address: usize,
        allocation_byte_len: usize,
        byte_offset: usize,
        byte_len: usize,
    ) -> Self {
        Self {
            declared_allocation_id,
            address_space,
            base_address,
            allocation_byte_len,
            byte_offset,
            byte_len,
        }
    }

    pub const fn declared_allocation_id(self) -> u64 {
        self.declared_allocation_id
    }

    pub const fn byte_offset(self) -> usize {
        self.byte_offset
    }

    pub const fn byte_len(self) -> usize {
        self.byte_len
    }

    fn validate(self, field: &'static str) -> Result<(), ScalarGemmHostAdmissionErrorV1> {
        if self.address_space != SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1 {
            return Err(ScalarGemmHostAdmissionErrorV1::WrongAddressSpace { field });
        }
        self.base_address
            .checked_add(self.allocation_byte_len)
            .ok_or(ScalarGemmHostAdmissionErrorV1::AllocationEndOverflow { field })?;
        let region_end = self
            .byte_offset
            .checked_add(self.byte_len)
            .ok_or(ScalarGemmHostAdmissionErrorV1::RegionEndOverflow { field })?;
        if region_end > self.allocation_byte_len {
            return Err(ScalarGemmHostAdmissionErrorV1::RegionOutOfBounds { field });
        }
        self.base_address
            .checked_add(region_end)
            .ok_or(ScalarGemmHostAdmissionErrorV1::PointerEndOverflow { field })?;
        Ok(())
    }

    fn overlaps(self, other: Self) -> bool {
        if self.declared_allocation_id != other.declared_allocation_id
            || self.byte_len == 0
            || other.byte_len == 0
        {
            return false;
        }
        let self_end = self.byte_offset + self.byte_len;
        let other_end = other.byte_offset + other.byte_len;
        self.byte_offset < other_end && other.byte_offset < self_end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarGemmToolchainV1 {
    UpstreamLlvmLld,
    Comgr,
    Other,
}

/// Declared source/profile preconditions that must pass before scalar GEMM V1
/// can be staged. The target, profile, roots, calls, and toolchain fields are
/// declarations, not observations of emitted HSACO or executed tools.
#[derive(Clone, Copy, Debug)]
pub struct ScalarGemmHostRequestV1<'a> {
    pub m: u32,
    pub n: u32,
    pub k: u32,
    pub a_len: usize,
    pub b_len: usize,
    pub c_len: usize,
    pub a_region: ScalarGemmBufferRegionV1,
    pub b_region: ScalarGemmBufferRegionV1,
    pub c_region: ScalarGemmBufferRegionV1,
    pub declared_target: &'a str,
    pub declared_coverage_profile: &'a str,
    pub declared_root_symbols: &'a [&'a str],
    pub declared_called_symbols: &'a [&'a str],
    pub declared_toolchain: ScalarGemmToolchainV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmHostAdmissionV1 {
    shape: ScalarGemmShapeV1,
}

impl ScalarGemmHostAdmissionV1 {
    pub const fn shape(self) -> ScalarGemmShapeV1 {
        self.shape
    }

    pub const fn grants_compiler_authority(self) -> bool {
        false
    }

    pub const fn proves_gpu_execution(self) -> bool {
        false
    }

    pub const fn proves_source_to_machine_refinement(self) -> bool {
        false
    }

    pub const fn attests_emitted_hsaco_target(self) -> bool {
        false
    }

    pub const fn attests_actual_toolchain_execution(self) -> bool {
        false
    }

    pub const fn attests_launch_domain_authentication(self) -> bool {
        false
    }

    pub const fn attests_allocation_provenance(self) -> bool {
        false
    }

    pub const fn proves_physical_non_aliasing(self) -> bool {
        false
    }
}

pub fn admit_scalar_gemm_host_v1(
    request: ScalarGemmHostRequestV1<'_>,
) -> Result<ScalarGemmHostAdmissionV1, ScalarGemmHostAdmissionErrorV1> {
    let shape = ScalarGemmShapeV1::checked(request.m, request.n, request.k)?;
    if shape.rounded_grid_threads() > SCALAR_GEMM_MAX_GRID_THREADS_V1 {
        return Err(ScalarGemmHostAdmissionErrorV1::LaunchDomainOverflow {
            output_elements: shape.c_elements,
        });
    }
    require_length("A", request.a_len, shape.a_len)?;
    require_length("B", request.b_len, shape.b_len)?;
    require_length("C", request.c_len, shape.c_len)?;
    require_region("A", request.a_region, shape.a_bytes)?;
    require_region("B", request.b_region, shape.b_bytes)?;
    require_region("C", request.c_region, shape.c_bytes)?;
    require_consistent_allocation_metadata("A", request.a_region, "B", request.b_region)?;
    require_consistent_allocation_metadata("A", request.a_region, "C", request.c_region)?;
    require_consistent_allocation_metadata("B", request.b_region, "C", request.c_region)?;

    if request.c_region.overlaps(request.a_region) {
        return Err(ScalarGemmHostAdmissionErrorV1::OutputAliasesInput { input: "A" });
    }
    if request.c_region.overlaps(request.b_region) {
        return Err(ScalarGemmHostAdmissionErrorV1::OutputAliasesInput { input: "B" });
    }
    if request.declared_target != SCALAR_GEMM_TARGET_V1 {
        return Err(ScalarGemmHostAdmissionErrorV1::WrongTarget);
    }
    if request.declared_coverage_profile != SCALAR_GEMM_COVERAGE_PROFILE_V1 {
        return Err(ScalarGemmHostAdmissionErrorV1::WrongCoverageProfile);
    }
    if request.declared_toolchain != ScalarGemmToolchainV1::UpstreamLlvmLld {
        return Err(match request.declared_toolchain {
            ScalarGemmToolchainV1::Comgr => ScalarGemmHostAdmissionErrorV1::ComgrForbidden,
            _ => ScalarGemmHostAdmissionErrorV1::WrongToolchain,
        });
    }
    if request.declared_root_symbols != [SCALAR_GEMM_ROOT_SYMBOL_V1] {
        return Err(ScalarGemmHostAdmissionErrorV1::WrongRootSet);
    }
    if !request.declared_called_symbols.is_empty() {
        return Err(ScalarGemmHostAdmissionErrorV1::UnexpectedCalls);
    }

    Ok(ScalarGemmHostAdmissionV1 { shape })
}

fn require_length(
    field: &'static str,
    observed: usize,
    expected: usize,
) -> Result<(), ScalarGemmHostAdmissionErrorV1> {
    if observed != expected {
        return Err(ScalarGemmHostAdmissionErrorV1::LengthMismatch {
            field,
            expected,
            observed,
        });
    }
    Ok(())
}

fn require_region(
    field: &'static str,
    region: ScalarGemmBufferRegionV1,
    expected: usize,
) -> Result<(), ScalarGemmHostAdmissionErrorV1> {
    region.validate(field)?;
    if region.byte_len != expected {
        return Err(ScalarGemmHostAdmissionErrorV1::RegionLengthMismatch {
            field,
            expected,
            observed: region.byte_len,
        });
    }
    Ok(())
}

fn require_consistent_allocation_metadata(
    left_field: &'static str,
    left: ScalarGemmBufferRegionV1,
    right_field: &'static str,
    right: ScalarGemmBufferRegionV1,
) -> Result<(), ScalarGemmHostAdmissionErrorV1> {
    if left.declared_allocation_id == right.declared_allocation_id
        && (left.address_space != right.address_space
            || left.base_address != right.base_address
            || left.allocation_byte_len != right.allocation_byte_len)
    {
        return Err(ScalarGemmHostAdmissionErrorV1::AllocationIdentityMismatch {
            left: left_field,
            right: right_field,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmInvocationV1 {
    p: u64,
    row: u32,
    col: u32,
}

impl ScalarGemmInvocationV1 {
    pub const fn p(self) -> u64 {
        self.p
    }

    pub const fn row(self) -> u32 {
        self.row
    }

    pub const fn col(self) -> u32 {
        self.col
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmDotStepV1 {
    t: u32,
    a_index: u64,
    b_index: u64,
}

impl ScalarGemmDotStepV1 {
    pub const fn t(self) -> u32 {
        self.t
    }

    pub const fn a_index(self) -> u64 {
        self.a_index
    }

    pub const fn b_index(self) -> u64 {
        self.b_index
    }
}

/// Lazy, ordered witness for exactly `k` recurrence steps. Step `t` denotes
/// `acc[t+1] = add(acc[t], mul(A[row*k+t], B[t*n+col]))`.
#[derive(Clone, Debug)]
pub struct ScalarGemmDotRecurrenceV1 {
    shape: ScalarGemmShapeV1,
    invocation: ScalarGemmInvocationV1,
    next_t: u32,
}

impl ScalarGemmDotRecurrenceV1 {
    pub const fn invocation(&self) -> ScalarGemmInvocationV1 {
        self.invocation
    }
}

impl Iterator for ScalarGemmDotRecurrenceV1 {
    type Item = ScalarGemmDotStepV1;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_t >= self.shape.k {
            return None;
        }
        let t = self.next_t;
        self.next_t += 1;
        let row = u64::from(self.invocation.row);
        let col = u64::from(self.invocation.col);
        let k = u64::from(self.shape.k);
        let n = u64::from(self.shape.n);
        Some(ScalarGemmDotStepV1 {
            t,
            a_index: row * k + u64::from(t),
            b_index: u64::from(t) * n + col,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.shape.k - self.next_t) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ScalarGemmDotRecurrenceV1 {}

pub fn scalar_gemm_flattened_index_is_correct_v1(shape: ScalarGemmShapeV1, p: u64) -> bool {
    match shape.invocation(p) {
        Some(invocation) => {
            u64::from(invocation.row) * u64::from(shape.n) + u64::from(invocation.col) == p
                && invocation.row < shape.m
                && invocation.col < shape.n
        }
        None => p >= shape.c_elements,
    }
}

pub fn scalar_gemm_accesses_are_in_bounds_v1(shape: ScalarGemmShapeV1, p: u64, t: u32) -> bool {
    let Some(invocation) = shape.invocation(p) else {
        return true;
    };
    if t >= shape.k {
        return true;
    }
    let a_index = u64::from(invocation.row) * u64::from(shape.k) + u64::from(t);
    let b_index = u64::from(t) * u64::from(shape.n) + u64::from(invocation.col);
    a_index < shape.a_elements && b_index < shape.b_elements && p < shape.c_elements
}

pub fn scalar_gemm_writers_are_injective_v1(
    shape: ScalarGemmShapeV1,
    left_p: u64,
    right_p: u64,
) -> bool {
    match (shape.invocation(left_p), shape.invocation(right_p)) {
        (Some(left), Some(right)) if left_p != right_p => left.p != right.p,
        _ => true,
    }
}

pub fn scalar_gemm_output_initialized_by_invocation_v1(
    shape: ScalarGemmShapeV1,
    p: u64,
    output_index: u64,
) -> bool {
    shape
        .invocation(p)
        .is_some_and(|invocation| output_index < shape.c_elements && invocation.p == output_index)
}

pub fn scalar_gemm_complete_launch_initializes_output_v1(
    shape: ScalarGemmShapeV1,
    output_index: u64,
) -> bool {
    output_index < shape.c_elements
        && scalar_gemm_output_initialized_by_invocation_v1(shape, output_index, output_index)
}

/// Evaluates the fixed recurrence over caller-supplied abstract arithmetic.
/// The operation order is observable: one multiply and then one add for each
/// `t` in ascending order, starting from the supplied positive-zero model.
pub fn evaluate_scalar_gemm_abstract_invocation_v1<T, Multiply, Add>(
    shape: ScalarGemmShapeV1,
    p: u64,
    a: &[T],
    b: &[T],
    positive_zero: T,
    mut multiply: Multiply,
    mut add: Add,
) -> Result<Option<T>, ScalarGemmModelErrorV1>
where
    Multiply: FnMut(&T, &T) -> T,
    Add: FnMut(T, T) -> T,
{
    require_model_length("A", a.len(), shape.a_len)?;
    require_model_length("B", b.len(), shape.b_len)?;
    let Some(steps) = shape.dot_recurrence(p) else {
        return Ok(None);
    };
    let mut accumulator = positive_zero;
    for step in steps {
        let product = multiply(&a[step.a_index as usize], &b[step.b_index as usize]);
        accumulator = add(accumulator, product);
    }
    Ok(Some(accumulator))
}

/// Deterministic CPU oracle for the source recurrence. This models source-level
/// `f32` only; proving that generated gfx942 code neither contracts nor
/// reassociates these operations is a separate machine-refinement obligation.
pub fn scalar_gemm_f32_oracle_v1(
    shape: ScalarGemmShapeV1,
    a: &[f32],
    b: &[f32],
    c: &mut [f32],
) -> Result<(), ScalarGemmModelErrorV1> {
    require_model_length("A", a.len(), shape.a_len)?;
    require_model_length("B", b.len(), shape.b_len)?;
    require_model_length("C", c.len(), shape.c_len)?;
    for (p, output) in c.iter_mut().enumerate() {
        let mut accumulator = 0.0_f32;
        let steps = shape
            .dot_recurrence(p as u64)
            .expect("p is within the checked output extent");
        for step in steps {
            let product =
                oracle_separate_f32_mul(a[step.a_index as usize], b[step.b_index as usize]);
            accumulator = oracle_separate_f32_add(accumulator, product);
        }
        *output = accumulator;
    }
    Ok(())
}

#[inline(never)]
fn oracle_separate_f32_mul(left: f32, right: f32) -> f32 {
    std::hint::black_box(std::hint::black_box(left) * std::hint::black_box(right))
}

#[inline(never)]
fn oracle_separate_f32_add(left: f32, right: f32) -> f32 {
    std::hint::black_box(std::hint::black_box(left) + std::hint::black_box(right))
}

fn require_model_length(
    field: &'static str,
    observed: usize,
    expected: usize,
) -> Result<(), ScalarGemmModelErrorV1> {
    if observed != expected {
        return Err(ScalarGemmModelErrorV1::LengthMismatch {
            field,
            expected,
            observed,
        });
    }
    Ok(())
}

/// Explicit limitation boundary for scalar GEMM V1's numerical claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmF32NumericalContractV1 {
    pub initial_accumulator_bits: u32,
    pub sequential_t_order: bool,
    pub reassociation_permitted: bool,
    pub contraction_permitted: bool,
    pub ieee_754_refinement_proved: bool,
    pub source_to_machine_refinement_proved: bool,
}

pub const SCALAR_GEMM_F32_NUMERICAL_CONTRACT_V1: ScalarGemmF32NumericalContractV1 =
    ScalarGemmF32NumericalContractV1 {
        initial_accumulator_bits: 0.0_f32.to_bits(),
        sequential_t_order: true,
        reassociation_permitted: false,
        contraction_permitted: false,
        ieee_754_refinement_proved: false,
        source_to_machine_refinement_proved: false,
    };

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarGemmHostAdmissionErrorV1 {
    ElementCountOverflow {
        field: &'static str,
    },
    ByteCountOverflow {
        field: &'static str,
    },
    HostSizeOverflow {
        field: &'static str,
    },
    LengthMismatch {
        field: &'static str,
        expected: usize,
        observed: usize,
    },
    RegionLengthMismatch {
        field: &'static str,
        expected: usize,
        observed: usize,
    },
    RegionEndOverflow {
        field: &'static str,
    },
    AllocationEndOverflow {
        field: &'static str,
    },
    PointerEndOverflow {
        field: &'static str,
    },
    RegionOutOfBounds {
        field: &'static str,
    },
    WrongAddressSpace {
        field: &'static str,
    },
    AllocationIdentityMismatch {
        left: &'static str,
        right: &'static str,
    },
    LaunchDomainOverflow {
        output_elements: u64,
    },
    OutputAliasesInput {
        input: &'static str,
    },
    WrongTarget,
    WrongCoverageProfile,
    ComgrForbidden,
    WrongToolchain,
    WrongRootSet,
    UnexpectedCalls,
}

impl fmt::Display for ScalarGemmHostAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElementCountOverflow { field } => {
                write!(formatter, "{field} element count overflow")
            }
            Self::ByteCountOverflow { field } => write!(formatter, "{field} byte count overflow"),
            Self::HostSizeOverflow { field } => {
                write!(formatter, "{field} extent is not host-representable")
            }
            Self::LengthMismatch {
                field,
                expected,
                observed,
            } => write!(
                formatter,
                "{field} length mismatch: expected {expected}, observed {observed}"
            ),
            Self::RegionLengthMismatch {
                field,
                expected,
                observed,
            } => write!(
                formatter,
                "{field} region length mismatch: expected {expected}, observed {observed}"
            ),
            Self::RegionEndOverflow { field } => write!(formatter, "{field} region end overflow"),
            Self::AllocationEndOverflow { field } => {
                write!(formatter, "{field} allocation end overflow")
            }
            Self::PointerEndOverflow { field } => write!(formatter, "{field} pointer end overflow"),
            Self::RegionOutOfBounds { field } => {
                write!(formatter, "{field} region is out of bounds")
            }
            Self::WrongAddressSpace { field } => {
                write!(formatter, "{field} is not in the global address space")
            }
            Self::AllocationIdentityMismatch { left, right } => write!(
                formatter,
                "{left} and {right} disagree about one allocation identity"
            ),
            Self::LaunchDomainOverflow { output_elements } => write!(
                formatter,
                "{output_elements} outputs do not fit the declared one-dimensional grid"
            ),
            Self::OutputAliasesInput { input } => write!(formatter, "C aliases {input}"),
            Self::WrongTarget => formatter.write_str("scalar GEMM V1 requires exact gfx942:xnack-"),
            Self::WrongCoverageProfile => formatter.write_str("scalar GEMM V1 requires COV6"),
            Self::ComgrForbidden => formatter.write_str("COMGR is forbidden for scalar GEMM V1"),
            Self::WrongToolchain => {
                formatter.write_str("scalar GEMM V1 requires upstream LLVM/LLD")
            }
            Self::WrongRootSet => {
                formatter.write_str("scalar GEMM V1 requires one exact kernel root")
            }
            Self::UnexpectedCalls => formatter.write_str("scalar GEMM V1 permits no call roots"),
        }
    }
}

impl std::error::Error for ScalarGemmHostAdmissionErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarGemmModelErrorV1 {
    LengthMismatch {
        field: &'static str,
        expected: usize,
        observed: usize,
    },
}

impl fmt::Display for ScalarGemmModelErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthMismatch {
                field,
                expected,
                observed,
            } => write!(
                formatter,
                "{field} model length mismatch: expected {expected}, observed {observed}"
            ),
        }
    }
}

impl std::error::Error for ScalarGemmModelErrorV1 {}

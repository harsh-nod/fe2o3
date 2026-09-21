use std::fmt::{self, Write as _};

use crate::{
    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
    AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE, AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME,
    AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE,
    AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME, AddressSpace, AmdGpuDiagnosticOperation,
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    FloatIntrinsicCapabilityV1, FloatOperation, Operation, OperationKind, SynchronizationScope,
    TargetCapability, WaveWidth,
};

#[path = "capability_borrowed_key_v1.rs"]
mod borrowed_key;

/// Allocation-free view of an extension capability name.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum TargetCapabilityNameRefV1<'a> {
    Text(&'a str),
    LowerHex(&'a [u8]),
}

impl TargetCapabilityNameRefV1<'_> {
    pub fn visible_len(self) -> Option<usize> {
        match self {
            Self::Text(value) => Some(value.len()),
            Self::LowerHex(bytes) => bytes.len().checked_mul(2),
        }
    }

    pub fn matches(self, candidate: &str) -> bool {
        match self {
            Self::Text(value) => value == candidate,
            Self::LowerHex(bytes) => {
                candidate.len() == bytes.len().saturating_mul(2)
                    && bytes.iter().zip(candidate.as_bytes().chunks_exact(2)).all(
                        |(byte, encoded)| {
                            encoded[0] == lower_hex_digit(byte >> 4)
                                && encoded[1] == lower_hex_digit(byte & 0x0f)
                        },
                    )
            }
        }
    }

    fn into_owned(self) -> String {
        match self {
            Self::Text(value) => value.to_owned(),
            Self::LowerHex(bytes) => {
                let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
                for byte in bytes {
                    encoded.push(char::from(lower_hex_digit(byte >> 4)));
                    encoded.push(char::from(lower_hex_digit(byte & 0x0f)));
                }
                encoded
            }
        }
    }
}

impl fmt::Debug for TargetCapabilityNameRefV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Text(value) => value.fmt(formatter),
            Self::LowerHex(bytes) => {
                formatter.write_str("\"")?;
                for byte in bytes {
                    formatter.write_char(char::from(lower_hex_digit(byte >> 4)))?;
                    formatter.write_char(char::from(lower_hex_digit(byte & 0x0f)))?;
                }
                formatter.write_str("\"")
            }
        }
    }
}

const fn lower_hex_digit(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        _ => b'a' + nibble - 10,
    }
}

/// The contextual Atomic requirement used by function capability derivation.
/// This inert classifier neither verifies the pointer nor grants atomic access.
pub fn atomic_pointer_capability_v1(
    atomic: &crate::Atomic,
    pointer_type: &crate::Type,
) -> Option<TargetCapability> {
    let crate::Type::Pointer(pointer) = pointer_type else {
        return None;
    };
    let width_bits = pointer
        .pointee
        .as_scalar()
        .and_then(crate::ScalarType::bit_width)?;
    matches!(width_bits, 8 | 16 | 32 | 64).then_some(TargetCapability::Atomic {
        width_bits,
        address_space: atomic.access.address_space,
        max_scope: atomic.scope,
    })
}

#[cfg(test)]
#[path = "descriptor_capability_visitation_v1_tests.rs"]
mod descriptor_tests;

/// Allocation-free view of one required target capability.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum TargetCapabilityRefV1<'a> {
    Float16,
    BFloat16,
    Float64,
    Int64,
    Subgroups,
    SubgroupSize(u32),
    WorkgroupMemory,
    WorkgroupBarrier,
    Atomic {
        width_bits: u16,
        address_space: AddressSpace,
        max_scope: SynchronizationScope,
    },
    DynamicWorkgroupMemory,
    Extension {
        namespace: &'a str,
        name: TargetCapabilityNameRefV1<'a>,
    },
    WaveWidth(WaveWidth),
}

impl fmt::Debug for TargetCapabilityRefV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Float16 => formatter.write_str("Float16"),
            Self::BFloat16 => formatter.write_str("BFloat16"),
            Self::Float64 => formatter.write_str("Float64"),
            Self::Int64 => formatter.write_str("Int64"),
            Self::Subgroups => formatter.write_str("Subgroups"),
            Self::SubgroupSize(size) => formatter.debug_tuple("SubgroupSize").field(size).finish(),
            Self::WorkgroupMemory => formatter.write_str("WorkgroupMemory"),
            Self::WorkgroupBarrier => formatter.write_str("WorkgroupBarrier"),
            Self::Atomic {
                width_bits,
                address_space,
                max_scope,
            } => formatter
                .debug_struct("Atomic")
                .field("width_bits", width_bits)
                .field("address_space", address_space)
                .field("max_scope", max_scope)
                .finish(),
            Self::DynamicWorkgroupMemory => formatter.write_str("DynamicWorkgroupMemory"),
            Self::Extension { namespace, name } => formatter
                .debug_struct("Extension")
                .field("namespace", namespace)
                .field("name", name)
                .finish(),
            Self::WaveWidth(width) => formatter.debug_tuple("WaveWidth").field(width).finish(),
        }
    }
}

impl<'a> TargetCapabilityRefV1<'a> {
    pub fn from_owned(capability: &'a TargetCapability) -> Self {
        match capability {
            TargetCapability::Float16 => Self::Float16,
            TargetCapability::BFloat16 => Self::BFloat16,
            TargetCapability::Float64 => Self::Float64,
            TargetCapability::Int64 => Self::Int64,
            TargetCapability::Subgroups => Self::Subgroups,
            TargetCapability::SubgroupSize(size) => Self::SubgroupSize(*size),
            TargetCapability::WorkgroupMemory => Self::WorkgroupMemory,
            TargetCapability::WorkgroupBarrier => Self::WorkgroupBarrier,
            TargetCapability::Atomic {
                width_bits,
                address_space,
                max_scope,
            } => Self::Atomic {
                width_bits: *width_bits,
                address_space: *address_space,
                max_scope: *max_scope,
            },
            TargetCapability::DynamicWorkgroupMemory => Self::DynamicWorkgroupMemory,
            TargetCapability::Extension { namespace, name } => Self::Extension {
                namespace,
                name: TargetCapabilityNameRefV1::Text(name),
            },
            TargetCapability::WaveWidth(width) => Self::WaveWidth(*width),
        }
    }

    pub(crate) fn extension(namespace: &'a str, name: &'a str) -> Self {
        Self::Extension {
            namespace,
            name: TargetCapabilityNameRefV1::Text(name),
        }
    }

    pub(crate) fn extension_lower_hex(namespace: &'a str, bytes: &'a [u8]) -> Self {
        Self::Extension {
            namespace,
            name: TargetCapabilityNameRefV1::LowerHex(bytes),
        }
    }

    pub(crate) fn matches(self, candidate: &TargetCapability) -> bool {
        match (self, candidate) {
            (Self::Float16, TargetCapability::Float16)
            | (Self::BFloat16, TargetCapability::BFloat16)
            | (Self::Float64, TargetCapability::Float64)
            | (Self::Int64, TargetCapability::Int64)
            | (Self::Subgroups, TargetCapability::Subgroups)
            | (Self::WorkgroupMemory, TargetCapability::WorkgroupMemory)
            | (Self::WorkgroupBarrier, TargetCapability::WorkgroupBarrier)
            | (Self::DynamicWorkgroupMemory, TargetCapability::DynamicWorkgroupMemory) => true,
            (Self::SubgroupSize(left), TargetCapability::SubgroupSize(right)) => left == *right,
            (Self::WaveWidth(left), TargetCapability::WaveWidth(right)) => left == *right,
            (
                Self::Atomic {
                    width_bits: left_width,
                    address_space: left_space,
                    max_scope: left_scope,
                },
                TargetCapability::Atomic {
                    width_bits: right_width,
                    address_space: right_space,
                    max_scope: right_scope,
                },
            ) => {
                left_width == *right_width
                    && left_space == *right_space
                    && left_scope == *right_scope
            }
            (
                Self::Extension { namespace, name },
                TargetCapability::Extension {
                    namespace: candidate_namespace,
                    name: candidate_name,
                },
            ) => namespace == candidate_namespace && name.matches(candidate_name),
            _ => false,
        }
    }

    /// Applies the verifier's asymmetric target-support policy without
    /// materializing this required capability.
    pub(crate) fn is_satisfied_by_v1(self, candidate: &TargetCapability) -> bool {
        match self {
            Self::Atomic {
                width_bits,
                address_space,
                max_scope,
            } => matches!(
                candidate,
                TargetCapability::Atomic {
                    width_bits: candidate_width,
                    address_space: candidate_space,
                    max_scope: candidate_scope,
                } if *candidate_width == width_bits
                    && *candidate_space == address_space
                    && candidate_scope.rank() >= max_scope.rank()
            ),
            Self::Extension { namespace, name }
                if namespace == AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
                    && name.matches(AMDGPU_DIAGNOSTICS_CAPABILITY_NAME) =>
            {
                self.matches(candidate)
                    || matches!(
                        candidate,
                        TargetCapability::Extension { namespace, name }
                            if namespace == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
                                && name == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME
                    )
            }
            Self::Extension { namespace, name }
                if namespace == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
                    && name.matches(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME) =>
            {
                self.matches(candidate)
                    || matches!(
                        candidate,
                        TargetCapability::Extension { namespace, name }
                            if namespace == AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
                                && name == AMDGPU_DIAGNOSTICS_CAPABILITY_NAME
                    )
            }
            _ => self.matches(candidate),
        }
    }

    /// Upper-bounds one complete support-policy comparison, including AMDGPU
    /// diagnostic alias classification and the terminal decision.
    pub(crate) fn support_comparison_work_v1(self, candidate: &TargetCapability) -> Option<usize> {
        let fixed = 4_usize;
        match (self, candidate) {
            (
                Self::Extension { namespace, name },
                TargetCapability::Extension {
                    namespace: candidate_namespace,
                    name: candidate_name,
                },
            ) => {
                let namespace_width = namespace
                    .len()
                    .max(candidate_namespace.len())
                    .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len())
                    .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.len());
                let name_width = name
                    .visible_len()?
                    .max(candidate_name.len())
                    .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len())
                    .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.len());
                // At most two required-alias guards, followed by exact and
                // alternate-alias candidate checks. Each can inspect both
                // dynamic strings and its terminal decision.
                namespace_width
                    .checked_add(name_width)?
                    .checked_add(2)?
                    .checked_mul(4)
                    .and_then(|work| work.checked_add(fixed))
            }
            (Self::Extension { namespace, name }, _) => {
                let namespace_width = namespace
                    .len()
                    .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len())
                    .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.len());
                let name_width = name
                    .visible_len()?
                    .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len())
                    .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.len());
                namespace_width
                    .checked_add(name_width)?
                    .checked_add(2)?
                    .checked_mul(2)
                    .and_then(|work| work.checked_add(fixed))
            }
            _ => Some(fixed),
        }
    }

    pub(crate) fn into_owned(self) -> TargetCapability {
        match self {
            Self::Float16 => TargetCapability::Float16,
            Self::BFloat16 => TargetCapability::BFloat16,
            Self::Float64 => TargetCapability::Float64,
            Self::Int64 => TargetCapability::Int64,
            Self::Subgroups => TargetCapability::Subgroups,
            Self::SubgroupSize(size) => TargetCapability::SubgroupSize(size),
            Self::WorkgroupMemory => TargetCapability::WorkgroupMemory,
            Self::WorkgroupBarrier => TargetCapability::WorkgroupBarrier,
            Self::Atomic {
                width_bits,
                address_space,
                max_scope,
            } => TargetCapability::Atomic {
                width_bits,
                address_space,
                max_scope,
            },
            Self::DynamicWorkgroupMemory => TargetCapability::DynamicWorkgroupMemory,
            Self::Extension { namespace, name } => TargetCapability::Extension {
                namespace: namespace.to_owned(),
                name: name.into_owned(),
            },
            Self::WaveWidth(width) => TargetCapability::WaveWidth(width),
        }
    }
}

pub(crate) fn target_capability_is_supported_with_budget_v1(
    required: TargetCapabilityRefV1<'_>,
    supported: &std::collections::BTreeSet<TargetCapability>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    // Empty sets and immediate misses retain a visible query action.
    budget.charge_work(1)?;
    if matches!(required, TargetCapabilityRefV1::Atomic { .. }) {
        // Atomic support is a scope-coverage policy, not exact key equality.
        for candidate in supported {
            budget.charge_work(
                required
                    .support_comparison_work_v1(candidate)
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
            )?;
            if required.is_satisfied_by_v1(candidate) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if supported.is_empty() {
        return Ok(false);
    }
    if borrowed_capability_contains_with_budget_v1(required, supported, budget)? {
        return Ok(true);
    }
    if !matches!(required, TargetCapabilityRefV1::Extension { .. }) {
        return Ok(false);
    }
    budget.charge_work(diagnostic_alias_classification_work_v1(required)?)?;
    let Some(alias) = diagnostic_alias_v1(required) else {
        return Ok(false);
    };
    budget.charge_work(1)?;
    borrowed_capability_contains_with_budget_v1(alias, supported, budget)
}

fn borrowed_capability_comparison_work_v1(
    required: TargetCapabilityRefV1<'_>,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    match required {
        TargetCapabilityRefV1::Extension { namespace, name } => {
            let bytes = name
                .visible_len()
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            // Exact Text/Text comparison observes at most the query's bytes.
            // Lazy hex additionally generates each compared name byte once.
            let name_work = match name {
                TargetCapabilityNameRefV1::Text(_) => bytes,
                TargetCapabilityNameRefV1::LowerHex(_) => bytes
                    .checked_mul(2)
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
            };
            namespace
                .len()
                .checked_add(name_work)
                .and_then(|work| work.checked_add(3))
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
        }
        _ => Ok(4),
    }
}

fn borrowed_capability_contains_with_budget_v1(
    required: TargetCapabilityRefV1<'_>,
    supported: &std::collections::BTreeSet<TargetCapability>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    let work = owned_btree_query_comparison_bound_v1(supported.len())?
        .checked_mul(borrowed_capability_comparison_work_v1(required)?)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.charge_work(work)?;
    Ok(borrowed_key::contains_v1(required, supported))
}

fn owned_capability_comparison_work_v1(
    capability: &TargetCapability,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    match capability {
        TargetCapability::Extension { namespace, name } => namespace
            .len()
            .checked_add(name.len())
            .and_then(|work| work.checked_add(3))
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic),
        _ => Ok(4),
    }
}

fn owned_btree_query_comparison_bound_v1(
    population: usize,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    // The workspace pins nightly-2026-04-03. Its alloc::collections::btree has
    // 11 keys per node and at least 5 keys in every non-root node. `contains`
    // scans each key in one node at most once, then follows one child. Thus a
    // non-empty tree with another path node needs at least 6*N+5 keys, and a
    // query compares at most 11 keys per visited node (and never more than the
    // total population). Keep these constants coupled to the pinned toolchain.
    const NODE_CAPACITY: usize = 11;
    const MIN_NON_ROOT_KEYS: usize = 5;

    if population == 0 {
        return Ok(0);
    }
    let mut path_nodes = 1_usize;
    let mut minimum_keys = 1_usize;
    while let Some(next_minimum_keys) = minimum_keys
        .checked_mul(MIN_NON_ROOT_KEYS + 1)
        .and_then(|keys| keys.checked_add(MIN_NON_ROOT_KEYS))
    {
        if next_minimum_keys > population {
            break;
        }
        minimum_keys = next_minimum_keys;
        path_nodes = path_nodes
            .checked_add(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    }
    Ok(population.min(
        path_nodes
            .checked_mul(NODE_CAPACITY)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    ))
}

fn owned_capability_contains_with_budget_v1(
    required: &TargetCapability,
    supported: &std::collections::BTreeSet<TargetCapability>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    // Preserve the owned BTreeSet lookup and charge its pinned implementation
    // bound, plus one action for an empty or terminal query.
    let work = owned_btree_query_comparison_bound_v1(supported.len())?
        .checked_mul(owned_capability_comparison_work_v1(required)?)
        .and_then(|work| work.checked_add(1))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.charge_work(work)?;
    Ok(supported.contains(required))
}

fn diagnostic_alias_v1(
    required: TargetCapabilityRefV1<'_>,
) -> Option<TargetCapabilityRefV1<'static>> {
    let TargetCapabilityRefV1::Extension { namespace, name } = required else {
        return None;
    };
    if namespace == AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
        && name.matches(AMDGPU_DIAGNOSTICS_CAPABILITY_NAME)
    {
        Some(TargetCapabilityRefV1::extension(
            AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
            AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
        ))
    } else if namespace == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
        && name.matches(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME)
    {
        Some(TargetCapabilityRefV1::extension(
            AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
            AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
        ))
    } else {
        None
    }
}

fn diagnostic_alias_classification_work_v1(
    required: TargetCapabilityRefV1<'_>,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    let TargetCapabilityRefV1::Extension { namespace, name } = required else {
        return Ok(1);
    };
    let namespace_width = namespace
        .len()
        .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len())
        .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.len());
    let name_bytes = name
        .visible_len()
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let name_work = match name {
        TargetCapabilityNameRefV1::Text(_) => name_bytes,
        TargetCapabilityNameRefV1::LowerHex(_) => name_bytes
            .checked_mul(2)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    };
    let name_width = name_work
        .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len())
        .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.len());
    namespace_width
        .checked_add(name_width)
        .and_then(|work| work.checked_add(2))
        .and_then(|work| work.checked_mul(2))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
}

pub(crate) fn target_capability_is_supported_owned_with_budget_v1(
    required: &TargetCapability,
    supported: &std::collections::BTreeSet<TargetCapability>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(1)?;
    if matches!(required, TargetCapability::Atomic { .. }) {
        return target_capability_is_supported_with_budget_v1(
            TargetCapabilityRefV1::from_owned(required),
            supported,
            budget,
        );
    }
    if owned_capability_contains_with_budget_v1(required, supported, budget)? {
        return Ok(true);
    }
    let required = TargetCapabilityRefV1::from_owned(required);
    budget.charge_work(diagnostic_alias_classification_work_v1(required)?)?;
    let Some(alias) = diagnostic_alias_v1(required) else {
        return Ok(false);
    };
    budget.charge_work(1)?;
    borrowed_capability_contains_with_budget_v1(alias, supported, budget)
}

impl Operation {
    /// Work required to classify this operation's capability roster before
    /// the first visitor callback. Callers separately charge one publication
    /// action per yielded capability.
    pub fn required_capability_visitation_work_v1(&self) -> Option<usize> {
        match &self.kind {
            OperationKind::Barrier(barrier) => {
                1_usize.checked_add(barrier.semantics.address_spaces.len())
            }
            OperationKind::Fence(fence) => {
                1_usize.checked_add(fence.semantics.address_spaces.len())
            }
            OperationKind::WorkgroupBarrier(barrier) => {
                1_usize.checked_add(barrier.semantics.address_spaces.len())
            }
            OperationKind::Matrix(_) => Some(16),
            OperationKind::Call { callee, .. } => {
                AmdGpuDiagnosticOperation::intrinsic_descriptor_lookup_work_v1(callee)?
                    .checked_add(FloatOperation::intrinsic_descriptor_lookup_work_v1(callee)?)?
                    .checked_add(1)
            }
            _ => Some(1),
        }
    }

    /// Visits the same sorted, duplicate-free capability roster returned by
    /// `required_capabilities` without constructing owned strings or a set.
    pub fn try_visit_required_capabilities_v1<E>(
        &self,
        mut visitor: impl FnMut(TargetCapabilityRefV1<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        match &self.kind {
            OperationKind::Intrinsic(_) | OperationKind::MemoryIntrinsic(_) => {}
            OperationKind::Alloca {
                count,
                address_space: AddressSpace::Workgroup,
                ..
            } => {
                visitor(TargetCapabilityRefV1::WorkgroupMemory)?;
                if count.is_some() {
                    visitor(TargetCapabilityRefV1::DynamicWorkgroupMemory)?;
                }
            }
            OperationKind::Barrier(barrier) => {
                match barrier.execution_scope {
                    SynchronizationScope::Subgroup => {
                        visitor(TargetCapabilityRefV1::Subgroups)?;
                    }
                    SynchronizationScope::Workgroup => {
                        if barrier
                            .semantics
                            .address_spaces
                            .contains(&AddressSpace::Workgroup)
                        {
                            visitor(TargetCapabilityRefV1::WorkgroupMemory)?;
                        }
                        visitor(TargetCapabilityRefV1::WorkgroupBarrier)?;
                        return Ok(());
                    }
                    _ => {}
                }
                visit_workgroup_memory_capability_v1(
                    &barrier.semantics.address_spaces,
                    &mut visitor,
                )?;
            }
            OperationKind::Fence(fence) => {
                if fence.memory_scope == SynchronizationScope::Subgroup {
                    visitor(TargetCapabilityRefV1::Subgroups)?;
                }
                visit_workgroup_memory_capability_v1(
                    &fence.semantics.address_spaces,
                    &mut visitor,
                )?;
            }
            OperationKind::WorkgroupBarrier(barrier) => {
                visit_workgroup_memory_capability_v1(
                    &barrier.semantics.address_spaces,
                    &mut visitor,
                )?;
                visitor(TargetCapabilityRefV1::WorkgroupBarrier)?;
            }
            OperationKind::WorkgroupMemory(memory) => {
                visitor(TargetCapabilityRefV1::WorkgroupMemory)?;
                if memory.extent.is_dynamic() {
                    visitor(TargetCapabilityRefV1::DynamicWorkgroupMemory)?;
                }
            }
            OperationKind::Matrix(matrix) => {
                matrix.try_visit_required_capabilities_v1(visitor)?;
            }
            OperationKind::Gfx950LdsTranspose(_) => {
                visitor(TargetCapabilityRefV1::Subgroups)?;
                visitor(TargetCapabilityRefV1::SubgroupSize(64))?;
                visitor(TargetCapabilityRefV1::WorkgroupMemory)?;
                visitor(TargetCapabilityRefV1::extension(
                    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                    AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME,
                ))?;
                visitor(TargetCapabilityRefV1::WaveWidth(WaveWidth::Wave64))?;
            }
            OperationKind::Wave(wave) => {
                visitor(TargetCapabilityRefV1::Subgroups)?;
                visitor(TargetCapabilityRefV1::SubgroupSize(wave.width.lanes()))?;
                visitor(TargetCapabilityRefV1::WaveWidth(wave.width))?;
            }
            OperationKind::Gfx942OrderedRegion(_) => {
                visitor(TargetCapabilityRefV1::extension(
                    crate::AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAMESPACE,
                    crate::AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAME,
                ))?;
                visitor(TargetCapabilityRefV1::extension(
                    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                    crate::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
                ))?;
                visitor(TargetCapabilityRefV1::WaveWidth(WaveWidth::Wave64))?;
            }
            OperationKind::Gfx942OrderedProgram(_) => {
                visitor(TargetCapabilityRefV1::extension(
                    crate::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAMESPACE,
                    crate::AMDGPU_GFX942_ORDERED_PROGRAM_CAPABILITY_NAME,
                ))?;
                visitor(TargetCapabilityRefV1::extension(
                    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE,
                    crate::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
                ))?;
                visitor(TargetCapabilityRefV1::WaveWidth(WaveWidth::Wave64))?;
            }
            OperationKind::InlineAssembly(_) => {
                visitor(TargetCapabilityRefV1::extension(
                    AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE,
                    AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME,
                ))?;
            }
            OperationKind::Call { callee, arguments } => {
                if AmdGpuDiagnosticOperation::intrinsic_descriptor_v1(callee)
                    .is_some_and(|descriptor| descriptor.arity() == arguments.len())
                {
                    visitor(TargetCapabilityRefV1::extension(
                        AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
                        AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
                    ))?;
                } else if let Some(descriptor) = FloatOperation::intrinsic_descriptor_v1(callee)
                    && descriptor.arity() == arguments.len()
                {
                    match descriptor.capability() {
                        FloatIntrinsicCapabilityV1::None => {}
                        FloatIntrinsicCapabilityV1::Float16 => {
                            visitor(TargetCapabilityRefV1::Float16)?;
                        }
                        FloatIntrinsicCapabilityV1::BFloat16 => {
                            visitor(TargetCapabilityRefV1::BFloat16)?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn visit_workgroup_memory_capability_v1<E>(
    address_spaces: &std::collections::BTreeSet<AddressSpace>,
    visitor: &mut impl FnMut(TargetCapabilityRefV1<'_>) -> Result<(), E>,
) -> Result<(), E> {
    if address_spaces.contains(&AddressSpace::Workgroup) {
        visitor(TargetCapabilityRefV1::WorkgroupMemory)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "operation_capability_visitation_v1_tests.rs"]
mod tests;

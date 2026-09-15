//! Exact borrowed executable-coordinate identity across capability additions.
//! This relation does not identify a target or grant executable authority.

use std::{cmp::Ordering, collections::BTreeSet, error::Error as StdError, fmt, mem::size_of};

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, Kernel, Module,
    TargetCapability, VerifiedCanonicalKernelIrModuleV12 as Owner,
};

/// Exact non-capability fields and coordinates of the two actual V12 owners.
/// Capability sets may only grow; the AMD binder adapter checks the exact delta.
#[derive(Debug)]
pub struct CheckedCanonicalKirCoordinatePreservationV1<'input, 'output> {
    input: &'input Owner,
    output: &'output Owner,
}

impl<'input, 'output> CheckedCanonicalKirCoordinatePreservationV1<'input, 'output> {
    /// The actual admitted input whose executable coordinates were checked.
    pub const fn input(&self) -> &'input Owner {
        self.input
    }
    /// The actual admitted successor, with identical non-capability fields.
    pub const fn output(&self) -> &'output Owner {
        self.output
    }
    /// Coordinate preservation alone grants no execution or proof authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Logical borrowed-view payload; both actual graph owners remain caller-owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirCoordinatePreservationStorageV1(usize);

impl CanonicalKirCoordinatePreservationStorageV1 {
    /// Logical bytes to reserve while retaining the borrowed checked view.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// A resource refusal or exact coordinate-preservation mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirCoordinatePreservationErrorV1 {
    /// The shared caller ledger rejected work, storage or arithmetic.
    Resource(Resource),
    /// A complete executable field or capability-extension rule differed.
    Mismatch(&'static str),
}

impl From<Resource> for CanonicalKirCoordinatePreservationErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CanonicalKirCoordinatePreservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Mismatch(rule) => {
                write!(formatter, "canonical coordinate identity rejected: {rule}")
            }
        }
    }
}
impl StdError for CanonicalKirCoordinatePreservationErrorV1 {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Mismatch(_) => None,
        }
    }
}

type Error = CanonicalKirCoordinatePreservationErrorV1;
type Result<T> = std::result::Result<T, Error>;

/// Checks complete executable fields without cloning, decoding or renumbering.
/// The existing typed IR equality compares every body field, including operands,
/// terminators and edge arguments. Its full variable traversal is prepaid using
/// both already-admitted canonical lengths; capability merge comparisons have
/// additional explicit charges because one key can participate more than once.
///
/// Success transfers only the checked borrowed view. As with the transition
/// checker, the incoming storage floor is restored on every returned result;
/// reserve the returned receipt while retaining the view.
pub fn check_canonical_kir_coordinate_preservation_v1<'input, 'output>(
    input: &'input Owner,
    output: &'output Owner,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirCoordinatePreservationV1<'input, 'output>,
    CanonicalKirCoordinatePreservationStorageV1,
)> {
    let floor = budget.storage();
    let retained = size_of::<CheckedCanonicalKirCoordinatePreservationV1<'_, '_>>();
    let result = (|| {
        budget.charge_work(1)?;
        budget.reserve_storage(retained)?;
        // This is a logical complete-payload comparison prepayment, not a hash
        // comparison or a claim to measure allocator/runtime instruction work.
        let payload_work = input
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(output.canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(payload_work)?;
        // Exhaustive outer destructuring makes new declaration fields require
        // an explicit decision; complete derived body equality covers its fields.
        let Module {
            id,
            functions,
            kernels,
            required_capabilities,
        } = input.module();
        let Module {
            id: output_id,
            functions: output_functions,
            kernels: output_kernels,
            required_capabilities: output_capabilities,
        } = output.module();
        budget.charge_work(3)?;
        if id != output_id
            || functions.len() != output_functions.len()
            || kernels.len() != output_kernels.len()
        {
            return Err(Error::Mismatch("module identity or cardinality"));
        }
        require_capability_extension(required_capabilities, output_capabilities, budget)?;
        for (a, b) in kernels.iter().zip(output_kernels) {
            let Kernel {
                id,
                entry,
                domain,
                workgroup_size,
                required_capabilities,
            } = a;
            let Kernel {
                id: output_id,
                entry: output_entry,
                domain: output_domain,
                workgroup_size: output_workgroup_size,
                required_capabilities: output_capabilities,
            } = b;
            budget.charge_work(4)?;
            if id != output_id
                || entry != output_entry
                || domain != output_domain
                || workgroup_size != output_workgroup_size
            {
                return Err(Error::Mismatch("kernel declaration"));
            }
            require_capability_extension(required_capabilities, output_capabilities, budget)?;
        }
        for (a, b) in functions.iter().zip(output_functions) {
            let Function {
                id,
                signature,
                role,
                body,
                required_capabilities,
            } = a;
            let Function {
                id: output_id,
                signature: output_signature,
                role: output_role,
                body: output_body,
                required_capabilities: output_capabilities,
            } = b;
            budget.charge_work(4)?;
            if id != output_id
                || role != output_role
                || signature != output_signature
                || body != output_body
            {
                return Err(Error::Mismatch("function declaration or executable body"));
            }
            require_capability_extension(required_capabilities, output_capabilities, budget)?;
        }
        Ok(CheckedCanonicalKirCoordinatePreservationV1 { input, output })
    })();
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(release)?;
    result.map(|checked| {
        (
            checked,
            CanonicalKirCoordinatePreservationStorageV1(retained),
        )
    })
}

fn capability_comparison_work(a: &TargetCapability, b: &TargetCapability) -> Result<usize> {
    let size = |capability: &TargetCapability| -> Result<usize> {
        match capability {
            TargetCapability::Extension { namespace, name } => namespace
                .len()
                .checked_add(name.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic.into()),
            _ => Ok(1),
        }
    };
    size(a)?
        .checked_add(size(b)?)
        .ok_or(Resource::Arithmetic.into())
}

fn require_capability_extension(
    input: &BTreeSet<TargetCapability>,
    output: &BTreeSet<TargetCapability>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(1)?;
    let mut input = input.iter();
    let mut next_input = input.next();
    for candidate in output {
        budget.charge_work(1)?;
        let Some(required) = next_input else {
            continue;
        };
        budget.charge_work(capability_comparison_work(required, candidate)?)?;
        match required.cmp(candidate) {
            Ordering::Less => return Err(Error::Mismatch("removed capability")),
            Ordering::Equal => next_input = input.next(),
            Ordering::Greater => {}
        }
    }
    budget.charge_work(1)?;
    if next_input.is_some() {
        return Err(Error::Mismatch("removed capability"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "canonical_kir_coordinate_preservation_v1_tests.rs"]
mod tests;

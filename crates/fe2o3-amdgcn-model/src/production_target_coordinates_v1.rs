//! Independent exact metadata check for the existing production target binder.
//! This is coordinate custody only, not target execution or formal authority.

use std::{cmp::Ordering, collections::BTreeSet, error::Error as StdError, fmt};

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_analysis::{
    CanonicalKirCoordinatePreservationErrorV1, CanonicalKirCoordinatePreservationStorageV1,
    CheckedCanonicalKirCoordinatePreservationV1, check_canonical_kir_coordinate_preservation_v1,
};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Module, TargetCapability,
    VerifiedCanonicalKernelIrModuleV12 as Owner, WaveWidth,
};

/// Failure of executable identity or the exact production target metadata delta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionTargetCoordinateErrorV1 {
    /// The caller's shared resource ledger refused the check.
    Resource(Resource),
    /// Non-capability executable fields or coordinates changed.
    Coordinates(CanonicalKirCoordinatePreservationErrorV1),
    /// Capabilities differed from the exact admitted target-binding rule.
    Metadata(&'static str),
}
impl From<Resource> for ProductionTargetCoordinateErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<CanonicalKirCoordinatePreservationErrorV1> for ProductionTargetCoordinateErrorV1 {
    fn from(value: CanonicalKirCoordinatePreservationErrorV1) -> Self {
        Self::Coordinates(value)
    }
}
impl fmt::Display for ProductionTargetCoordinateErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Coordinates(error) => error.fmt(formatter),
            Self::Metadata(rule) => {
                write!(formatter, "production target metadata rejected: {rule}")
            }
        }
    }
}
impl StdError for ProductionTargetCoordinateErrorV1 {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Coordinates(error) => Some(error),
            Self::Metadata(_) => None,
        }
    }
}
type Error = ProductionTargetCoordinateErrorV1;
type Result<T> = std::result::Result<T, Error>;

/// Checks the actual admitted N/B pair against exactly the existing binder's
/// capability delta. It neither calls the binder as an oracle nor changes B.
/// Production orchestration still constructs B using the real binder.
///
/// The borrowed neutral-coordinate receipt is transferred with its existing
/// logical payload; no extra owned graph, target strings or capability set is
/// allocated. All returned paths restore the incoming storage floor.
pub fn check_production_target_coordinate_preservation_v1<'neutral, 'bound>(
    neutral: &'neutral Owner,
    bound: &'bound Owner,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirCoordinatePreservationV1<'neutral, 'bound>,
    CanonicalKirCoordinatePreservationStorageV1,
)> {
    let floor = budget.storage();
    let result = (|| {
        budget.charge_work(1)?;
        let (coordinates, storage) =
            check_canonical_kir_coordinate_preservation_v1(neutral, bound, budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        let a = neutral.module();
        let b = bound.module();
        budget.charge_work(1)?;
        if a.kernels.is_empty() {
            return Err(Error::Metadata("missing kernel"));
        }
        let target = match profile {
            ProductionAmdTargetProfileV1::Gfx942 => {
                AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
            }
            ProductionAmdTargetProfileV1::Gfx950 => {
                AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME
            }
        };
        exact_capabilities(
            &a.required_capabilities,
            &b.required_capabilities,
            true,
            target,
            budget,
        )?;
        for (a, b) in a.kernels.iter().zip(&b.kernels) {
            budget.charge_work(1)?;
            exact_capabilities(
                &a.required_capabilities,
                &b.required_capabilities,
                true,
                target,
                budget,
            )?;
        }
        for (a, b) in a.functions.iter().zip(&b.functions) {
            budget.charge_work(1)?;
            let entry = kernel_entry(neutral.module(), a.id.as_str(), budget)?;
            exact_capabilities(
                &a.required_capabilities,
                &b.required_capabilities,
                entry,
                target,
                budget,
            )?;
        }
        Ok((coordinates, storage))
    })();
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(release)?;
    result
}

fn kernel_entry(module: &Module, function: &str, budget: &mut Budget<'_>) -> Result<bool> {
    for kernel in &module.kernels {
        budget.charge_work(1)?;
        budget.charge_work(
            kernel
                .entry
                .as_str()
                .len()
                .checked_add(function.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if kernel.entry.as_str() == function {
            return Ok(true);
        }
    }
    Ok(false)
}

fn comparison_work(a: &TargetCapability, b: &TargetCapability) -> Result<usize> {
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

fn is_target(capability: &TargetCapability, target: &str, budget: &mut Budget<'_>) -> Result<bool> {
    budget.charge_work(1)?;
    let TargetCapability::Extension { namespace, name } = capability else {
        return Ok(false);
    };
    let bytes = namespace
        .len()
        .checked_add(name.len())
        .and_then(|n| n.checked_add(AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.len()))
        .and_then(|n| n.checked_add(target.len()))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(bytes)?;
    Ok(namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE && name == target)
}

fn exact_capabilities(
    input: &BTreeSet<TargetCapability>,
    output: &BTreeSet<TargetCapability>,
    add_target: bool,
    target: &str,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(1)?;
    let mut original = input.iter();
    let mut required = original.next();
    let mut saw_target = false;
    let mut saw_wave64 = false;
    for candidate in output {
        budget.charge_work(1)?;
        let target_matches = is_target(candidate, target, budget)?;
        budget.charge_work(1)?;
        let wave_matches = matches!(candidate, TargetCapability::WaveWidth(WaveWidth::Wave64));
        saw_target |= target_matches;
        saw_wave64 |= wave_matches;
        let matched_original = if let Some(expected) = required {
            budget.charge_work(comparison_work(expected, candidate)?)?;
            match expected.cmp(candidate) {
                Ordering::Less => return Err(Error::Metadata("removed required capability")),
                Ordering::Equal => {
                    required = original.next();
                    true
                }
                Ordering::Greater => false,
            }
        } else {
            false
        };
        budget.charge_work(1)?;
        if !matched_original && !(add_target && (target_matches || wave_matches)) {
            return Err(Error::Metadata("unexpected capability addition"));
        }
    }
    budget.charge_work(2)?;
    if required.is_some() || (add_target && !(saw_target && saw_wave64)) {
        return Err(Error::Metadata("incomplete target capability delta"));
    }
    Ok(())
}

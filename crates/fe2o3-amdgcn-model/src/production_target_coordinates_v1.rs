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

/// Exact supplied native N/B binding plus unchanged graph-bound catalogs.
///
/// This is a borrowed conjunction, not evidence that the binder executed.
/// Catalog source identities remain inert until independently authenticated
/// against the original source. No source, hardware or publication authority
/// follows from this relation, including when N and B have equal identities.
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::CheckedNativeV12TargetBindingRelationV1 as R;
/// fn duplicate(relation: R<'_, '_, '_, '_>) { let _ = relation.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::CheckedNativeV12TargetBindingRelationV1 as R;
/// fn forge<'a>() -> R<'a, 'a, 'a, 'a> {
///     R { coordinates: todo!(), neutral_catalog: todo!(), bound_catalog: todo!(),
///         profile: todo!() }
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::{CheckedNativeV12TargetBindingRelationV1 as R,
///     check_native_v12_target_binding_relation_v1 as check};
/// use fe2o3_kernel_analysis::CheckedKernelIrContractCatalogV1 as C;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O,
///     CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// use fe2o3_amd_target::ProductionAmdTargetProfileV1 as P;
/// fn escape<'a>(n: O, b: &'a O, cn: &C<'a, 'a>, cb: &C<'a, 'a>, budget: &mut B<'_>)
///     -> R<'static, 'a, 'a, 'a> {
///     check(&n, cn, b, cb, P::Gfx942, budget).unwrap().0
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::{CheckedNativeV12TargetBindingRelationV1 as R,
///     check_native_v12_target_binding_relation_v1 as check};
/// use fe2o3_kernel_analysis::CheckedKernelIrContractCatalogV1 as C;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O,
///     CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// use fe2o3_amd_target::ProductionAmdTargetProfileV1 as P;
/// fn escape<'a>(n: &'a O, b: O, cn: &C<'a, 'a>, cb: &C<'a, 'a>, budget: &mut B<'_>)
///     -> R<'a, 'a, 'static, 'a> {
///     check(n, cn, &b, cb, P::Gfx942, budget).unwrap().0
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::{CheckedNativeV12TargetBindingRelationV1 as R,
///     check_native_v12_target_binding_relation_v1 as check};
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV1 as I,
///     CheckedKernelIrContractCatalogV1 as C, check_kernel_ir_contract_catalog_v1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O,
///     InertCanonicalKernelIrContractCatalogV1 as Catalog,
///     CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// use fe2o3_amd_target::ProductionAmdTargetProfileV1 as P;
/// fn escape<'a>(n: &'a O, b: &'a O, inv: &'a I<'a>, cn: Catalog,
///     cb: &C<'a, 'a>, budget: &mut B<'_>) -> R<'a, 'static, 'a, 'a> {
///     let (checked, _) = check_kernel_ir_contract_catalog_v1(inv, &cn, budget).unwrap();
///     check(n, &checked, b, cb, P::Gfx942, budget).unwrap().0
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::{CheckedNativeV12TargetBindingRelationV1 as R,
///     check_native_v12_target_binding_relation_v1 as check};
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV1 as I,
///     CheckedKernelIrContractCatalogV1 as C, check_kernel_ir_contract_catalog_v1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O,
///     InertCanonicalKernelIrContractCatalogV1 as Catalog,
///     CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// use fe2o3_amd_target::ProductionAmdTargetProfileV1 as P;
/// fn escape<'a>(n: &'a O, b: &'a O, inv: &'a I<'a>, cb: Catalog,
///     cn: &C<'a, 'a>, budget: &mut B<'_>) -> R<'a, 'a, 'a, 'static> {
///     let (checked, _) = check_kernel_ir_contract_catalog_v1(inv, &cb, budget).unwrap();
///     check(n, cn, b, &checked, P::Gfx942, budget).unwrap().0
/// }
/// ```
#[derive(Debug)]
pub struct CheckedNativeV12TargetBindingRelationV1<'n, 'cn, 'b, 'cb> {
    coordinates: CheckedCanonicalKirCoordinatePreservationV1<'n, 'b>,
    neutral_catalog: &'cn fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    bound_catalog: &'cb fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
    profile: ProductionAmdTargetProfileV1,
}

impl<'n, 'cn, 'b, 'cb> CheckedNativeV12TargetBindingRelationV1<'n, 'cn, 'b, 'cb> {
    /// The exact supplied admitted N owner, not a recreated lookalike.
    pub const fn neutral(&self) -> &'n Owner {
        self.coordinates.input()
    }
    /// The exact supplied admitted B owner, not a binder replay output.
    pub const fn bound(&self) -> &'b Owner {
        self.coordinates.output()
    }
    /// The unchanged existing coordinate-preservation result.
    pub const fn coordinates(&self) -> &CheckedCanonicalKirCoordinatePreservationV1<'n, 'b> {
        &self.coordinates
    }
    /// The exact immutable N catalog. Payload traversal remains caller-budgeted.
    pub const fn neutral_catalog(
        &self,
    ) -> &'cn fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1 {
        self.neutral_catalog
    }
    /// The exact immutable B catalog. Payload traversal remains caller-budgeted.
    pub const fn bound_catalog(
        &self,
    ) -> &'cb fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1 {
        self.bound_catalog
    }
    /// The closed target profile whose capability delta was checked.
    pub const fn profile(&self) -> ProductionAmdTargetProfileV1 {
        self.profile
    }
    /// This conjunction does not grant source, execution or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Inline relation-header transfer, excluding the caller-owned input payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeV12TargetBindingRelationStorageV1(usize);
impl NativeV12TargetBindingRelationStorageV1 {
    /// Reserve while retaining the relation, before further ledger allocation.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Failure of supplied graph/catalog custody or exact target binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeV12TargetBindingRelationErrorV1 {
    /// Existing caller-owned canonical resource accounting refused the operation.
    Resource(Resource),
    /// The unchanged independent N/B coordinate/target check failed.
    Target(ProductionTargetCoordinateErrorV1),
    /// A supplied graph owner or complete catalog content did not match.
    Invalid(&'static str),
}
impl From<Resource> for NativeV12TargetBindingRelationErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<ProductionTargetCoordinateErrorV1> for NativeV12TargetBindingRelationErrorV1 {
    fn from(error: ProductionTargetCoordinateErrorV1) -> Self {
        Self::Target(error)
    }
}
impl fmt::Display for NativeV12TargetBindingRelationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::Target(error) => error.fmt(formatter),
            Self::Invalid(reason) => {
                write!(formatter, "native N/B target relation rejected: {reason}")
            }
        }
    }
}
impl StdError for NativeV12TargetBindingRelationErrorV1 {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

/// Independently checks supplied native N/B and complete unchanged catalogs.
///
/// Both checked catalogs must already bind their exact supplied graph owners.
/// Catalog source identities are preserved, not authenticated here. The same
/// catalog object may be independently bound on both graphs; separately decoded
/// identical catalogs are also accepted. Equal N/B identities never skip checks.
/// No binder, optimizer, decoder, graph clone or inventory rebuild is invoked.
///
/// The caller retains every input owner/catalog/inventory/view receipt on its
/// original ledger. This adds no heap allocation. Work is the existing target
/// check plus 7 + both catalog byte lengths; the old checker retains its O(F*K)
/// worst-case kernel-entry scans. Only this fixed-size borrowed header transfers.
/// All returned paths restore the incoming storage floor and preserve history.
pub fn check_native_v12_target_binding_relation_v1<'n, 'cn, 'b, 'cb>(
    neutral: &'n Owner,
    neutral_catalog: &fe2o3_kernel_analysis::CheckedKernelIrContractCatalogV1<'cn, 'n>,
    bound: &'b Owner,
    bound_catalog: &fe2o3_kernel_analysis::CheckedKernelIrContractCatalogV1<'cb, 'b>,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> std::result::Result<
    (
        CheckedNativeV12TargetBindingRelationV1<'n, 'cn, 'b, 'cb>,
        NativeV12TargetBindingRelationStorageV1,
    ),
    NativeV12TargetBindingRelationErrorV1,
> {
    use NativeV12TargetBindingRelationErrorV1 as RelationError;
    let floor = budget.storage();
    let header = std::mem::size_of::<CheckedNativeV12TargetBindingRelationV1<'_, '_, '_, '_>>();
    let coordinate_header =
        std::mem::size_of::<CheckedCanonicalKirCoordinatePreservationV1<'_, '_>>();
    let result = (|| {
        budget.charge_work(4)?;
        budget.reserve_storage(
            header
                .checked_sub(coordinate_header)
                .ok_or(Resource::Accounting)?,
        )?;
        if !neutral_catalog.inventory().belongs_to(neutral)
            || !bound_catalog.inventory().belongs_to(bound)
        {
            return Err(RelationError::Invalid("catalog graph owner"));
        }
        let (coordinates, storage) =
            check_production_target_coordinate_preservation_v1(neutral, bound, profile, budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        budget.charge_work(1)?;
        if storage.retained_storage() != coordinate_header {
            return Err(Resource::Accounting.into());
        }
        let neutral_catalog = neutral_catalog.catalog();
        let bound_catalog = bound_catalog.catalog();
        let work = neutral_catalog
            .canonical_bytes()
            .len()
            .checked_add(bound_catalog.canonical_bytes().len())
            .and_then(|bytes| bytes.checked_add(2))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(work)?;
        if neutral_catalog.canonical_bytes() != bound_catalog.canonical_bytes() {
            return Err(RelationError::Invalid("complete catalog content"));
        }
        Ok(CheckedNativeV12TargetBindingRelationV1 {
            coordinates,
            neutral_catalog,
            bound_catalog,
            profile,
        })
    })();
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(release)?;
    result.map(|relation| (relation, NativeV12TargetBindingRelationStorageV1(header)))
}

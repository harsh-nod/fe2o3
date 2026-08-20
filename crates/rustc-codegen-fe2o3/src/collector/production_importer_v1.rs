//! Sole consuming boundary from production rustc collection to semantic MIR.

use std::collections::BTreeMap;
use std::fmt;

use fe2o3_mir_model::semantic_mir_v1::{
    HARD_MAX_FUNCTIONS_V1, HARD_MAX_ROOTS_V1, SemanticFunctionIdV1, SemanticFunctionIdentityV1,
    SemanticMirResourceV1, SemanticTargetDataLayoutV1,
};
use rustc_middle::ty::TyCtxt;

use super::{
    AuthenticatedCollectedKernelClosureV1, AuthenticatedProductionRootV1, CollectedFunctionRole,
    CollectionResult,
};
use crate::production_target_v1::ProductionTargetErrorV1;
use crate::rustc_semantic_adapter_v1::{
    CanonicalFunctionIdentitiesV1, SemanticIdentityDigestV1, canonical_function_identities_v1,
    canonical_target_layout_v1,
};

const IDENTITY_INVENTORY_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-identity-inventory/v1";

#[derive(Debug)]
pub(crate) enum ProductionSemanticImportErrorV1 {
    Target(ProductionTargetErrorV1),
    RootCustodyMismatch,
    LimitExceeded {
        resource: SemanticMirResourceV1,
        actual: u64,
        maximum: u64,
    },
    FunctionIdentityCollision,
    RootIdentityMismatch,
    SemanticRecordConstructionPending {
        collected_functions: usize,
        registered_roots: usize,
        llvm_target: String,
        rustc_identity_inventory_sha256: [u8; 32],
    },
}

impl fmt::Display for ProductionSemanticImportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(error) => write!(formatter, "semantic import target rejection: {error}"),
            Self::RootCustodyMismatch => formatter.write_str(
                "semantic importer rejected collector root custody before MIR construction",
            ),
            Self::LimitExceeded {
                resource,
                actual,
                maximum,
            } => write!(
                formatter,
                "semantic importer rejected {resource:?} count {actual} before semantic record allocation; maximum is {maximum}",
            ),
            Self::FunctionIdentityCollision => formatter.write_str(
                "semantic importer independently derived a duplicate canonical function identity",
            ),
            Self::RootIdentityMismatch => formatter.write_str(
                "semantic importer could not bind independently derived roots to unique collected functions",
            ),
            Self::SemanticRecordConstructionPending {
                collected_functions,
                registered_roots,
                llvm_target,
                rustc_identity_inventory_sha256,
            } => write!(
                formatter,
                "semantic importer authenticated rustc target {llvm_target:?}, consumed {collected_functions} collected device function(s) with {registered_roots} external root(s), and derived rustc identity inventory {}; but canonical semantic-MIR construction is not implemented; no fallback or artifact emission was entered",
                crate::encode_hex(rustc_identity_inventory_sha256),
            ),
        }
    }
}

impl std::error::Error for ProductionSemanticImportErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Target(error) => Some(error),
            Self::RootCustodyMismatch
            | Self::LimitExceeded { .. }
            | Self::FunctionIdentityCollision
            | Self::RootIdentityMismatch
            | Self::SemanticRecordConstructionPending { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SemanticFunctionIdentityInventoryEntryV1 {
    identities: CanonicalFunctionIdentitiesV1,
    role: CollectedFunctionRole,
}

#[derive(Debug)]
struct ProductionSemanticIdentityInventoryV1 {
    functions: Box<[SemanticFunctionIdentityInventoryEntryV1]>,
    roots: Box<[SemanticFunctionIdV1]>,
    sha256: [u8; 32],
}

/// Consumes the collector-sealed closure and authenticates the live rustc
/// session before any type, layout, FnAbi, or MIR fact can enter production.
///
/// This function intentionally stops after the identity-preflight milestone.
/// Canonical semantic-MIR construction will replace the terminal error without
/// introducing another consumer or returning the collected rustc values.
pub(crate) fn require_production_semantic_import_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
) -> ProductionSemanticImportErrorV1 {
    let AuthenticatedCollectedKernelClosureV1 {
        target,
        collection,
        roots,
    } = closure;
    let target = match target.authenticate_import_session(tcx) {
        Ok(target) => target,
        Err(error) => return ProductionSemanticImportErrorV1::Target(error),
    };
    let retained_roots = roots
        .iter()
        .map(|root| (root.instance, root.role, root.export_name.as_str()));
    let independently_observed_roots = collection
        .functions
        .iter()
        .filter(|function| {
            matches!(
                function.role,
                super::CollectedFunctionRole::KernelEntry
                    | super::CollectedFunctionRole::DeviceFfiExport
            )
        })
        .map(|function| {
            (
                function.instance,
                function.role,
                function.export_name.as_str(),
            )
        });
    if !exact_ordered_axes_match(retained_roots, independently_observed_roots) {
        return ProductionSemanticImportErrorV1::RootCustodyMismatch;
    }
    let identity_inventory = match build_identity_inventory_v1(tcx, &target, &collection, &roots) {
        Ok(inventory) => inventory,
        Err(error) => return error,
    };

    let error = ProductionSemanticImportErrorV1::SemanticRecordConstructionPending {
        collected_functions: identity_inventory.functions.len(),
        registered_roots: identity_inventory.roots.len(),
        llvm_target: target.rustc_layout().llvm_target().to_owned(),
        rustc_identity_inventory_sha256: identity_inventory.sha256,
    };
    drop((identity_inventory, target, collection, roots));
    error
}

fn build_identity_inventory_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    collection: &CollectionResult<'tcx>,
    roots: &[AuthenticatedProductionRootV1<'tcx>],
) -> Result<ProductionSemanticIdentityInventoryV1, ProductionSemanticImportErrorV1> {
    require_count_within_limit_v1(
        SemanticMirResourceV1::Functions,
        collection.functions.len(),
        HARD_MAX_FUNCTIONS_V1,
    )?;
    require_count_within_limit_v1(SemanticMirResourceV1::Roots, roots.len(), HARD_MAX_ROOTS_V1)?;

    let target = canonical_target_layout_v1(target.rustc_layout());
    let mut functions = Vec::with_capacity(collection.functions.len());
    for function in &collection.functions {
        functions.push(SemanticFunctionIdentityInventoryEntryV1 {
            identities: canonical_function_identities_v1(tcx, function.instance),
            role: function.role,
        });
    }
    functions.sort_unstable_by_key(|entry| entry.identities.function());
    if functions
        .windows(2)
        .any(|pair| pair[0].identities.function() == pair[1].identities.function())
    {
        return Err(ProductionSemanticImportErrorV1::FunctionIdentityCollision);
    }

    let mut function_ids = BTreeMap::<SemanticFunctionIdentityV1, SemanticFunctionIdV1>::new();
    for (index, function) in functions.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| ProductionSemanticImportErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Functions,
                actual: u64::MAX,
                maximum: HARD_MAX_FUNCTIONS_V1,
            })?;
        function_ids.insert(
            function.identities.function(),
            SemanticFunctionIdV1::from_index(index),
        );
    }

    let mut canonical_roots = Vec::with_capacity(roots.len());
    for root in roots {
        let identity = canonical_function_identities_v1(tcx, root.instance).function();
        let Some(function_id) = function_ids.get(&identity).copied() else {
            return Err(ProductionSemanticImportErrorV1::RootIdentityMismatch);
        };
        canonical_roots.push(function_id);
    }
    canonical_roots.sort_unstable();
    if canonical_roots.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProductionSemanticImportErrorV1::RootIdentityMismatch);
    }

    let sha256 = identity_inventory_sha256_v1(target, &functions, &canonical_roots);
    Ok(ProductionSemanticIdentityInventoryV1 {
        functions: functions.into_boxed_slice(),
        roots: canonical_roots.into_boxed_slice(),
        sha256,
    })
}

fn identity_inventory_sha256_v1(
    target: SemanticTargetDataLayoutV1,
    functions: &[SemanticFunctionIdentityInventoryEntryV1],
    roots: &[SemanticFunctionIdV1],
) -> [u8; 32] {
    let mut digest = SemanticIdentityDigestV1::new(IDENTITY_INVENTORY_DOMAIN_V1);
    digest.field(target.identity().as_bytes());
    for function in functions {
        digest.field(function.identities.function().as_bytes());
        digest.field(function.identities.item_definition().as_bytes());
        digest.field(function.identities.monomorphization().as_bytes());
        digest.field(function.identities.generic_type_arguments().as_bytes());
        digest.field(function.identities.const_generic_arguments().as_bytes());
        digest.field(&[function_role_tag_v1(function.role)]);
    }
    for root in roots {
        digest.field(&root.index().to_le_bytes());
    }
    digest.finish()
}

const fn function_role_tag_v1(role: CollectedFunctionRole) -> u8 {
    match role {
        CollectedFunctionRole::KernelEntry => 0,
        CollectedFunctionRole::InternalHelper => 1,
        CollectedFunctionRole::DeviceFfiExport => 2,
    }
}

fn require_count_within_limit_v1(
    resource: SemanticMirResourceV1,
    actual: usize,
    maximum: u64,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let actual = u64::try_from(actual).unwrap_or(u64::MAX);
    if actual > maximum {
        Err(ProductionSemanticImportErrorV1::LimitExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn exact_ordered_axes_match<T: PartialEq>(
    expected: impl IntoIterator<Item = T>,
    observed: impl IntoIterator<Item = T>,
) -> bool {
    expected.into_iter().eq(observed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_diagnostic_is_bounded_and_workload_neutral() {
        let error = ProductionSemanticImportErrorV1::SemanticRecordConstructionPending {
            collected_functions: 3,
            registered_roots: 2,
            llvm_target: "amdgcn-amd-amdhsa".to_owned(),
            rustc_identity_inventory_sha256: [0xab; 32],
        };
        let diagnostic = error.to_string();
        assert!(diagnostic.contains("3 collected device function(s)"));
        assert!(diagnostic.contains("2 external root(s)"));
        assert!(diagnostic.contains(&"ab".repeat(32)));
        for forbidden in [
            "GEMM",
            "attention",
            "softmax",
            "export name",
            concat!("MIR ", "transcript"),
        ] {
            assert!(!diagnostic.contains(forbidden));
        }
    }

    #[test]
    fn root_custody_comparison_rejects_every_sequence_substitution() {
        assert!(exact_ordered_axes_match([1, 2], [1, 2]));
        for substituted in [vec![1], vec![1, 2, 2], vec![2, 1], vec![1, 3]] {
            assert!(!exact_ordered_axes_match(vec![1, 2], substituted));
        }
    }

    #[test]
    fn count_preflight_rejects_before_semantic_record_allocation() {
        assert!(require_count_within_limit_v1(SemanticMirResourceV1::Functions, 4, 4).is_ok());
        assert!(matches!(
            require_count_within_limit_v1(SemanticMirResourceV1::Functions, 5, 4),
            Err(ProductionSemanticImportErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Functions,
                actual: 5,
                maximum: 4,
            })
        ));
    }
}

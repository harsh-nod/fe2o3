//! Sole consuming boundary from production rustc collection to semantic MIR.

use std::fmt;

use rustc_middle::ty::TyCtxt;

use super::AuthenticatedCollectedKernelClosureV1;
use crate::production_target_v1::ProductionTargetErrorV1;

#[derive(Debug)]
pub(crate) enum ProductionSemanticImportErrorV1 {
    Target(ProductionTargetErrorV1),
    RootCustodyMismatch,
    SemanticRecordConstructionPending {
        collected_functions: usize,
        registered_roots: usize,
        llvm_target: String,
    },
}

impl fmt::Display for ProductionSemanticImportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(error) => write!(formatter, "semantic import target rejection: {error}"),
            Self::RootCustodyMismatch => formatter.write_str(
                "semantic importer rejected collector root custody before MIR construction",
            ),
            Self::SemanticRecordConstructionPending {
                collected_functions,
                registered_roots,
                llvm_target,
            } => write!(
                formatter,
                "semantic importer authenticated rustc target {llvm_target:?} and consumed {collected_functions} collected device function(s) with {registered_roots} external root(s), but canonical semantic-MIR construction is not implemented; no fallback or artifact emission was entered",
            ),
        }
    }
}

impl std::error::Error for ProductionSemanticImportErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Target(error) => Some(error),
            Self::RootCustodyMismatch | Self::SemanticRecordConstructionPending { .. } => None,
        }
    }
}

/// Consumes the collector-sealed closure and authenticates the live rustc
/// session before any type, layout, FnAbi, or MIR fact can enter production.
///
/// This function intentionally stops after the target-authentication milestone.
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

    let error = ProductionSemanticImportErrorV1::SemanticRecordConstructionPending {
        collected_functions: collection.functions.len(),
        registered_roots: roots.len(),
        llvm_target: target.rustc_layout().llvm_target().to_owned(),
    };
    drop((target, collection, roots));
    error
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
        };
        let diagnostic = error.to_string();
        assert!(diagnostic.contains("3 collected device function(s)"));
        assert!(diagnostic.contains("2 external root(s)"));
        for forbidden in [
            "GEMM",
            "attention",
            "softmax",
            "export name",
            "MIR transcript",
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
}

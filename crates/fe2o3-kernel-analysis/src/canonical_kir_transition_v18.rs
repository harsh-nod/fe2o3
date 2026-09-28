//! The same independent scalar/CFG rules over actual storage-aware owners.
//! This establishes a rewrite relation, not final source or safety admission.

use super::{
    Budget, Candidate, CanonicalKirControlIndexStorageV1, CanonicalKirTransitionStorageV1,
    CheckedCanonicalKirControlIndexV1, CheckedCanonicalKirTransitionV1, Error, Result,
    check_transition,
};
use crate::CanonicalKirInventoryV18;
use fe2o3_kernel_ir::{
    StorageLayoutKindV1 as Kind, StorageLayoutV1, VerifiedCanonicalKernelIrModuleV18,
};

pub type CheckedCanonicalKirTransitionV18<'a, 'input, 'output, 'rows> =
    CheckedCanonicalKirTransitionV1<'a, 'input, 'output, 'rows, VerifiedCanonicalKernelIrModuleV18>;
pub type CheckedCanonicalKirControlIndexV18<'a, 'input, 'output> =
    CheckedCanonicalKirControlIndexV1<'a, 'input, 'output, VerifiedCanonicalKernelIrModuleV18>;

/// Checks the complete tables and actual input/output occurrence relation.
/// Both inventories, owners and candidate rows must remain paid and borrowed.
/// Success transfers only the borrowed view's explicit storage receipt. It
/// cannot transfer a source proof, authorize an unchecked pass, or become a V12
/// serialized receipt. All affected final analyses remain mandatory.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, check_canonical_kir_transition_v18};
/// use fe2o3_kernel_ir::{CanonicalKirTransitionCandidateV1, CanonicalKernelIrVerificationResourceBudgetV1};
/// fn legacy(a: &CanonicalKirInventoryV1<'_>, rows: CanonicalKirTransitionCandidateV1<'_>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = check_canonical_kir_transition_v18(a, a, rows, budget);
/// }
/// ```
pub fn check_canonical_kir_transition_v18<'a, 'input, 'output, 'rows>(
    input: &'a CanonicalKirInventoryV18<'input>,
    output: &'a CanonicalKirInventoryV18<'output>,
    rows: Candidate<'rows>,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirTransitionV18<'a, 'input, 'output, 'rows>,
    CanonicalKirTransitionStorageV1,
)> {
    let a = input.owner().module();
    let b = output.owner().module();
    if !same_table(&a.storage_layouts, &b.storage_layouts, budget)? {
        return Err(Error::Rule("complete storage layout table"));
    }
    check_transition(input, output, rows, a, b, budget)
}

impl<'a, 'input, 'output> CheckedCanonicalKirControlIndexV18<'a, 'input, 'output> {
    /// Derives transport queries only from this exact independently checked
    /// V18 relation. Its immutable tables cannot change during the borrow.
    pub fn derive_v18(
        checked: &CheckedCanonicalKirTransitionV18<'a, 'input, 'output, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirControlIndexStorageV1)> {
        Self::derive_modules(
            checked,
            checked.input().owner().module(),
            checked.output().owner().module(),
            budget,
        )
    }
}

// One finite table pass, not recursive object expansion. The row debit pays
// fixed metadata; each explicit variable-length field/variant is prepaid once.
fn same_table(
    a: &[StorageLayoutV1],
    b: &[StorageLayoutV1],
    budget: &mut Budget<'_>,
) -> Result<bool> {
    budget.charge_work(1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b) {
        budget.charge_work(1)?;
        if a.size != b.size || a.alignment != b.alignment {
            return Ok(false);
        }
        let same = match (&a.kind, &b.kind) {
            (Kind::Scalar(a), Kind::Scalar(b)) => a == b,
            (Kind::Vector(a), Kind::Vector(b)) => a == b,
            (Kind::Pointer(a), Kind::Pointer(b)) => a == b,
            (Kind::Record(a), Kind::Record(b)) | (Kind::Union(a), Kind::Union(b)) => {
                if a.len() != b.len() {
                    return Ok(false);
                }
                budget.charge_work(a.len())?;
                a == b
            }
            (
                Kind::Array {
                    element: a,
                    length: al,
                    stride: ast,
                },
                Kind::Array {
                    element: b,
                    length: bl,
                    stride: bst,
                },
            ) => a == b && al == bl && ast == bst,
            (
                Kind::Slice {
                    element: a,
                    value_space: av,
                    access: aa,
                    data: ad,
                    length: al,
                },
                Kind::Slice {
                    element: b,
                    value_space: bv,
                    access: ba,
                    data: bd,
                    length: bl,
                },
            ) => a == b && av == bv && aa == ba && ad == bd && al == bl,
            (
                Kind::Variants {
                    encoding: a,
                    variants: av,
                },
                Kind::Variants {
                    encoding: b,
                    variants: bv,
                },
            ) => {
                if a != b || av.len() != bv.len() {
                    return Ok(false);
                }
                budget.charge_work(av.len())?;
                av == bv
            }
            _ => false,
        };
        if !same {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "canonical_kir_transition_v18_resources_tests.rs"]
mod resources_tests;
#[cfg(test)]
#[path = "canonical_kir_transition_v18_tests.rs"]
mod tests;

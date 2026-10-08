//! Ordered CPU-origin descriptions projected from the retained binding owner.
use super::{AuthenticatedProductionBindings, Budget, Error, Resource, Root, policy_capture};
use crate::reference_effect_v1::ReferenceBindingOriginV1 as BindingOrigin;
use fe2o3_verifier::{
    NativeConditionalCpuExpectationV1 as Expectation,
    NativeConditionalCpuOriginExpectationV1 as Origin,
};
use std::mem::size_of;

/// The caller retains the original source and invocation beside these inert rows.
/// No constructor from CPU packet bytes or public provenance token is provided.
pub(super) struct RetainedCpuOriginRosterV1 {
    rows: Vec<Expectation>,
}

impl RetainedCpuOriginRosterV1 {
    pub(super) fn rows(&self) -> &[Expectation] {
        &self.rows
    }

    pub(super) fn retained_storage(&self) -> Result<usize, Resource> {
        self.rows
            .capacity()
            .checked_mul(size_of::<Expectation>())
            .and_then(|bytes| bytes.checked_add(size_of::<Self>()))
            .ok_or(Resource::Arithmetic)
    }
}

/// Called inside the original producer visit, after the packet assembler checks
/// the complete root/descriptor/source order. Terminal failures earn no refund.
pub(super) fn capture(
    bindings: &AuthenticatedProductionBindings,
    roots: &[Root],
    budget: &mut Budget<'_>,
) -> Result<RetainedCpuOriginRosterV1, Error> {
    let references = bindings.reference_effect_bindings.as_slice();
    budget.charge_work(3)?;
    if roots.is_empty()
        || roots.len() > fe2o3_compiler_lineage::MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_ROOTS_V1
        || roots.len() != references.len()
    {
        return Err(Error::Mismatch(
            "complete original CPU origin binding roster",
        ));
    }
    let mut rows = policy_capture::vector(roots.len(), budget)?;
    for root in roots {
        let origin = select(
            root.logical_name(),
            references
                .iter()
                .map(|binding| (binding.logical_kernel_name.as_str(), &binding.origin)),
            budget,
        )?;
        budget.charge_work(size_of::<Expectation>())?;
        rows.push(Expectation {
            semantic_root: root.semantic_root().index(),
            origin,
        });
    }
    Ok(RetainedCpuOriginRosterV1 { rows })
}

// Names select among already retained bindings, never among decoded CPU leaves.
// The bounded scan charges every comparison and rejects ambiguous source rows.
fn select<'a>(
    name: &str,
    references: impl IntoIterator<Item = (&'a str, &'a BindingOrigin)>,
    budget: &mut Budget<'_>,
) -> Result<Origin, Error> {
    let mut selected = None;
    for (candidate, origin) in references {
        budget.charge_work(
            name.len()
                .checked_add(candidate.len())
                .and_then(|bytes| bytes.checked_add(1))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if name != candidate {
            continue;
        }
        if selected.is_some() {
            return Err(Error::Mismatch("ambiguous original CPU origin binding"));
        }
        budget.charge_work(size_of::<Origin>() + 1)?;
        selected = Some(match origin {
            BindingOrigin::SourceRegistration(_) => Origin::SourceRegistrationV1,
            BindingOrigin::ReferenceEnrollment(origin) => Origin::ReferenceEnrollmentV1(*origin),
        });
    }
    selected.ok_or(Error::Mismatch("missing original CPU origin binding"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use fe2o3_verifier::portable_reference_v1::codec::ReferenceEnrollmentOriginV1;

    fn enrolled() -> BindingOrigin {
        BindingOrigin::ReferenceEnrollment(ReferenceEnrollmentOriginV1 {
            rustc_invocation_sha256: [1; 32],
            native_policy_sha256: [2; 32],
            policy_generation: 17,
            mapping_ordinal: 9,
        })
    }

    // These are inert projection checks, not fabricated issuer/admission tests.
    #[test]
    fn registration_uses_only_the_registration_codec() {
        let origin = BindingOrigin::SourceRegistration("crate::reference".into());
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 31);
        budget.reserve_storage(31).unwrap();
        assert_eq!(
            select("kernel", [("kernel", &origin)], &mut budget).unwrap(),
            Origin::SourceRegistrationV1
        );
        assert_eq!(budget.storage(), 31);
    }

    #[test]
    fn enrollment_preserves_all_original_fields() {
        let origin = enrolled();
        let BindingOrigin::ReferenceEnrollment(expected) = origin else {
            unreachable!()
        };
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 0);
        assert_eq!(
            select("kernel", [("kernel", &origin)], &mut budget).unwrap(),
            Origin::ReferenceEnrollmentV1(expected)
        );
    }

    #[test]
    fn mixed_bindings_select_by_exact_original_name() {
        let registration = BindingOrigin::SourceRegistration("crate::reference".into());
        let enrollment = enrolled();
        let rows = [("a", &registration), ("b", &enrollment)];
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 0);
        assert!(matches!(
            select("b", rows, &mut budget).unwrap(),
            Origin::ReferenceEnrollmentV1(_)
        ));
        assert_eq!(
            select("a", rows, &mut budget).unwrap(),
            Origin::SourceRegistrationV1
        );
    }

    #[test]
    fn missing_binding_has_no_registration_fallback() {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 0);
        assert!(matches!(
            select("kernel", [], &mut budget),
            Err(Error::Mismatch("missing original CPU origin binding"))
        ));
    }

    #[test]
    fn equal_duplicate_bindings_are_still_ambiguous() {
        let origin = enrolled();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 0);
        assert!(matches!(
            select(
                "kernel",
                [("kernel", &origin), ("kernel", &origin)],
                &mut budget
            ),
            Err(Error::Mismatch("ambiguous original CPU origin binding"))
        ));
    }

    #[test]
    fn name_prefixes_cannot_select_a_binding() {
        let origin = enrolled();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 0);
        assert!(matches!(
            select("kernel", [("kernel_extra", &origin)], &mut budget),
            Err(Error::Mismatch("missing original CPU origin binding"))
        ));
        assert_eq!(budget.work(), "kernel".len() + "kernel_extra".len() + 1);
    }

    #[test]
    fn exact_projection_work_preserves_the_original_account_and_floor() {
        let origin = enrolled();
        let cost = "kernel".len() * 2 + 1 + size_of::<Origin>() + 1;
        let mut work = Work::new(7 + cost);
        let mut budget = Budget::new(&mut work, 31);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(31).unwrap();
        let account = budget.work_ledger_identity_v1();
        select("kernel", [("kernel", &origin)], &mut budget).unwrap();
        assert_eq!(budget.work(), 7 + cost);
        assert_eq!(budget.storage(), 31);
        assert!(budget.work_ledger_identity_v1() == account);
    }

    #[test]
    fn repeated_projection_keeps_prefix_work_and_first_refusal() {
        let origin = enrolled();
        let comparison = "kernel".len() * 2 + 1;
        let cost = comparison + size_of::<Origin>() + 1;
        let mut work = Work::new(7 + cost * 2 - 1);
        let mut budget = Budget::new(&mut work, 31);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(31).unwrap();
        let account = budget.work_ledger_identity_v1();
        select("kernel", [("kernel", &origin)], &mut budget).unwrap();
        assert!(matches!(
            select("kernel", [("kernel", &origin)], &mut budget),
            Err(Error::Resource(_))
        ));
        assert_eq!(budget.work(), 7 + cost + comparison);
        let first_refusal = budget.failed_work();
        assert!(first_refusal.is_some());
        assert!(select("kernel", [("kernel", &origin)], &mut budget).is_err());
        assert_eq!(budget.failed_work(), first_refusal);
        assert_eq!(budget.storage(), 31);
        assert!(budget.work_ledger_identity_v1() == account);
    }
}

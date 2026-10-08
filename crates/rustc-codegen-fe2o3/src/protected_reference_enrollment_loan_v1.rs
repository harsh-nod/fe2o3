//! Scoped access to the original native session and admitted rustc invocation.
use super::{Error as SessionError, Policy};
use crate::protected_rustc_invocation::{
    AdmittedProtectedRustcInvocationV1 as Invocation, ReferenceEnrollmentInvocationStampV1,
    reference_enrollment_identity::{Owner, Stamp},
};
use crate::reference_effect_v1::ReferenceBindingErrorV1 as Error;
use crate::reference_enrollment_policy_v1::ReferenceEnrollmentRequestV1 as Request;
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1 as Work;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_rustc_invocation::RustcInvocationDescriptorV3;
use fe2o3_verifier::portable_reference_v1::codec::ReferenceEnrollmentOriginV1 as Origin;
use std::cell::{Cell, RefCell};

#[derive(Debug)]
pub(crate) struct RetainedReferenceEnrollmentStampV1 {
    session: Stamp,
    invocation: ReferenceEnrollmentInvocationStampV1,
}

type Revalidate<'a> =
    dyn FnMut(Option<&ReferenceEnrollmentInvocationStampV1>) -> Result<(), Error> + 'a;

/// No constructor or authority-bearing trait is available to the collector.
pub(crate) struct ReferenceEnrollmentLoanV1<'a> {
    descriptor: &'a RustcInvocationDescriptorV3,
    invocation: &'a Invocation,
    owner: &'a Owner,
    revalidate: RefCell<&'a mut Revalidate<'a>>,
    refused: Cell<bool>,
    requested_bindings: Cell<Option<usize>>,
    origin: Origin,
}

/// Borrowed only within the original client's checked preparation callback.
pub(crate) struct ReferenceEnrollmentPreparationV1<'a> {
    pub(super) policy: &'a Policy,
    pub(super) owner: Option<&'a Owner>,
    pub(super) ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

impl ReferenceEnrollmentPreparationV1<'_> {
    pub(crate) fn is_enrolled(&self) -> bool {
        self.owner.is_some()
    }

    pub(crate) fn with_invocation<T, E>(
        &self,
        invocation: &mut Invocation,
        budget: &mut Budget<'_>,
        run: impl FnOnce(&ReferenceEnrollmentLoanV1<'_>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<SessionError>,
    {
        if self.ledger != budget.work_ledger_identity_v1() {
            return Err(SessionError::Reference(Error::new(
                "reference enrollment original target account changed",
            ))
            .into());
        }
        let owner = self.owner.ok_or_else(|| {
            SessionError::Reference(Error::new("reference enrollment session was not retained"))
        })?;
        self.policy.revalidate(budget).map_err(SessionError::from)?;
        let original = invocation
            .retain_reference_enrollment_identity(budget)
            .map_err(SessionError::Reference)?;
        let rustc_invocation_sha256 = invocation
            .reference_enrollment_invocation_sha256(budget)
            .map_err(SessionError::Reference)?;
        let invocation = &*invocation;
        let origin = Origin {
            rustc_invocation_sha256,
            native_policy_sha256: *self.policy.policy().identity().as_bytes(),
            policy_generation: self.policy.policy().generation(),
            mapping_ordinal: 0,
        };
        let mut revalidate = |retained: Option<&ReferenceEnrollmentInvocationStampV1>| {
            self.policy
                .revalidate(budget)
                .map_err(|error| Error::new(error.to_string()))?;
            invocation
                .revalidate_reference_enrollment_identity(retained.unwrap_or(&original), budget)
        };
        let loan = ReferenceEnrollmentLoanV1 {
            descriptor: invocation.descriptor(),
            invocation,
            owner,
            revalidate: RefCell::new(&mut revalidate),
            refused: Cell::new(false),
            requested_bindings: Cell::new(None),
            origin,
        };
        let result = run(&loan);
        // An ignored refusal cannot be turned into a successful preparation.
        if loan.refused.get() {
            return Err(SessionError::Reference(Error::new(
                "reference enrollment loan was refused",
            ))
            .into());
        }
        let value = result?;
        loan.current(None).map_err(SessionError::Reference)?;
        Ok(value)
    }
}

impl ReferenceEnrollmentLoanV1<'_> {
    fn terminal<T>(&self, result: Result<T, Error>) -> Result<T, Error> {
        if result.is_err() {
            self.refused.set(true);
        }
        result
    }

    fn current(
        &self,
        retained: Option<&ReferenceEnrollmentInvocationStampV1>,
    ) -> Result<(), Error> {
        revalidate_live(&self.revalidate, &self.refused, retained)
    }

    fn charge(&self, work: &mut Work, amount: usize) -> Result<(), Error> {
        self.terminal(
            work.charge(amount)
                .map_err(|error| Error::new(error.to_string())),
        )
    }

    pub(crate) fn request(&self, work: &mut Work) -> Result<Option<Request>, Error> {
        self.charge(work, 1)?;
        self.current(None)?;
        let request = self.terminal(Request::from_descriptor(self.descriptor, work))?;
        self.requested_bindings
            .set(request.as_ref().map(|request| request.bindings().len()));
        Ok(request)
    }

    pub(crate) fn capture_stamp(
        &self,
        work: &mut Work,
    ) -> Result<RetainedReferenceEnrollmentStampV1, Error> {
        self.charge(
            work,
            std::mem::size_of::<RetainedReferenceEnrollmentStampV1>(),
        )?;
        self.current(None)?;
        Ok(RetainedReferenceEnrollmentStampV1 {
            session: self.owner.stamp(),
            invocation: self.terminal(self.invocation.reference_enrollment_stamp())?,
        })
    }

    pub(crate) fn revalidate_stamp(
        &self,
        stamp: &RetainedReferenceEnrollmentStampV1,
        work: &mut Work,
    ) -> Result<(), Error> {
        self.charge(work, 1)?;
        if !self.owner.matches(&stamp.session) {
            return self.terminal(Err(Error::new(
                "reference enrollment original native session changed",
            )));
        }
        self.current(Some(&stamp.invocation))
    }

    pub(crate) fn origin(&self, ordinal: u32) -> Result<Origin, Error> {
        self.current(None)?;
        if !self
            .requested_bindings
            .get()
            .is_some_and(|count| (ordinal as usize) < count)
        {
            return self.terminal(Err(Error::new(
                "reference enrollment ordinal is not in the captured request",
            )));
        }
        Ok(Origin {
            mapping_ordinal: ordinal,
            ..self.origin
        })
    }
}

fn revalidate_live(
    revalidate: &RefCell<&mut Revalidate<'_>>,
    refused: &Cell<bool>,
    retained: Option<&ReferenceEnrollmentInvocationStampV1>,
) -> Result<(), Error> {
    if refused.get() {
        return Err(Error::new("reference enrollment loan was refused"));
    }
    let result = revalidate
        .try_borrow_mut()
        .map_err(|_| Error::new("reference enrollment revalidation is reentrant"))
        .and_then(|mut revalidate| revalidate(retained));
    if result.is_err() {
        refused.set(true);
    }
    result?;
    if refused.get() {
        return Err(Error::new("reference enrollment loan was refused"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_currentness_check_visits_the_original_revalidator() {
        let visits = Cell::new(0);
        let refused = Cell::new(false);
        let mut check = |_: Option<&ReferenceEnrollmentInvocationStampV1>| {
            visits.set(visits.get() + 1);
            Ok(())
        };
        let revalidate = RefCell::new(&mut check as &mut Revalidate<'_>);
        revalidate_live(&revalidate, &refused, None).unwrap();
        revalidate_live(&revalidate, &refused, None).unwrap();
        assert_eq!(visits.get(), 2);
        assert!(!refused.get());
    }

    #[test]
    fn reentrant_check_refuses_without_panicking_or_retrying() {
        let visits = Cell::new(0);
        let refused = Cell::new(false);
        let mut check = |_: Option<&ReferenceEnrollmentInvocationStampV1>| {
            visits.set(visits.get() + 1);
            Ok(())
        };
        let revalidate = RefCell::new(&mut check as &mut Revalidate<'_>);
        let active = revalidate.borrow_mut();
        assert!(
            revalidate_live(&revalidate, &refused, None)
                .unwrap_err()
                .to_string()
                .contains("reentrant")
        );
        drop(active);
        assert!(revalidate_live(&revalidate, &refused, None).is_err());
        assert!(refused.get());
        assert_eq!(visits.get(), 0);
    }

    #[test]
    fn ignored_refusal_cannot_reenter_a_successful_revalidator() {
        let visits = Cell::new(0);
        let refused = Cell::new(false);
        let mut check = |_: Option<&ReferenceEnrollmentInvocationStampV1>| {
            visits.set(visits.get() + 1);
            refused.set(true);
            Ok(())
        };
        let revalidate = RefCell::new(&mut check as &mut Revalidate<'_>);
        assert!(revalidate_live(&revalidate, &refused, None).is_err());
        assert!(revalidate_live(&revalidate, &refused, None).is_err());
        assert_eq!(visits.get(), 1);
    }
}

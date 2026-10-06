//! The existing V2 local observation path; no V3 root-control fallback.
use super::*;
use std::marker::PhantomData;

#[derive(Default)]
pub(super) struct OccurrenceContext<'work>(PhantomData<&'work Budget<'work>>);

impl OccurrenceContext<'_> {
    pub(super) fn require_available(&self) -> Result<()> {
        Ok(())
    }

    pub(super) const fn retained_storage(&self) -> usize {
        0
    }

    pub(super) fn observe(
        &self,
        a: &Admission<'_>,
        b: &mut Budget<'_>,
    ) -> Result<(NativeOccurrence, usize)> {
        Ok(NativeOccurrence::observe(&a.service, b)?)
    }

    pub(super) fn validate(
        &self,
        a: &Admission<'_>,
        occurrence: &NativeOccurrence,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        Ok(occurrence.revalidate(&a.service, b)?)
    }

    pub(super) fn retire(
        &self,
        _a: &Admission<'_>,
        _occurrence: Option<&NativeOccurrence>,
        _carriage: &Carriage,
        _b: &mut Budget<'_>,
    ) -> Result<()> {
        // The shared session already performed the original local revalidation.
        // V2 has no root RPC and retains its existing retirement schedule.
        Ok(())
    }
}

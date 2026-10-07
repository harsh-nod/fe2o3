//! Live custody is session state, never reconstructed from a signed journal.
use super::*;

#[derive(Default)]
pub(super) struct Session<'work> {
    occurrence: Option<NativeOccurrence>,
    context: OccurrenceContext<'work>,
}

impl<'work> Session<'work> {
    pub(super) fn with_context(context: OccurrenceContext<'work>) -> Self {
        Self {
            occurrence: None,
            context,
        }
    }

    pub(super) fn require_context(&self, b: &mut Budget<'_>) -> Result<()> {
        b.charge_work(8)?;
        self.context.require_available()
    }
    pub(super) fn require_kind(&self, kind: Kind) -> Result<()> {
        self.context.require_kind(kind)
    }
    pub(super) fn require_carriage(&self, carriage: &Carriage) -> Result<()> {
        self.context.require_carriage(carriage)
    }
    pub fn publication_guard<'a>(
        &'a self,
        admission: &'a Admission<'a>,
        record: &Record,
    ) -> Result<PublicationGuard<'a, 'work>> {
        self.check_record(record)?;
        Ok(PublicationGuard {
            admission,
            occurrence: self.occurrence.as_ref(),
            context: &self.context,
        })
    }
    pub fn retained_storage(&self) -> usize {
        self.context.retained_storage()
            + self
                .occurrence
                .as_ref()
                .map_or(0, NativeOccurrence::retained_storage)
    }

    pub fn occurrence(&self) -> Result<&NativeOccurrence> {
        self.occurrence
            .as_ref()
            .ok_or_else(|| Error::rejected("native pending journal has no retained occurrence"))
    }

    pub fn prepare(
        &mut self,
        a: &Admission<'_>,
        record: &Record,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        self.validate(a, record, b)?;
        if !matches!(record.body, Body::Ready) {
            return Err(Error::rejected("prepare requires a ready native journal"));
        }
        let (occurrence, charge) = self.context.observe(a, b)?;
        b.reserve_storage(charge)?;
        self.occurrence = Some(occurrence);
        Ok(())
    }

    pub fn validate(&self, a: &Admission<'_>, record: &Record, b: &mut Budget<'_>) -> Result<()> {
        self.context.validate_service(a, b)?;
        self.check_record(record)?;
        if let Some(occurrence) = &self.occurrence {
            self.context.validate(a, occurrence, b)?;
        }
        Ok(())
    }

    pub(super) fn revalidate_occurrence(
        &self,
        a: &Admission<'_>,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        self.context.validate(a, self.occurrence()?, b)
    }

    fn check_record(&self, record: &Record) -> Result<()> {
        match (&record.body, &self.occurrence) {
            (Body::Ready, None) => Ok(()),
            (Body::Ready, Some(_)) => Err(Error::rejected(
                "native occurrence retirement is incomplete",
            )),
            (_, _) => {
                let occurrence = self.occurrence()?;
                matches_pending(record, occurrence.subject(), occurrence.identity())
            }
        }
    }

    /// Called only after the ledger has durably advanced and reacquired its exact
    /// Worker/anchor join. Drop the WHOLE occurrence (lease AND token) before an
    /// ACK permits the compiler to take the publication writer lock.
    pub fn retire(
        &mut self,
        a: &Admission<'_>,
        ledger: &Ledger,
        publication: &Publication,
        ack: &Ack,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let carriage = ledger.retirement_carriage(publication, ack, b)?;
        b.reserve_storage(carriage.retained_storage())?;
        if let Some(occurrence) = &self.occurrence {
            if publication.compiler_occurrence_identity() != *occurrence.identity()
                || carriage.request().subject().canonical_bytes()
                    != occurrence.subject().canonical_bytes()
            {
                return Err(Error::rejected("native retirement changed occurrence"));
            }
            self.context.validate(a, occurrence, b)?;
        }
        a.validate_continuity(b)?;
        self.context
            .retire(a, self.occurrence.as_ref(), &carriage, b)?;
        // Packet scopes retain their entry reservation. The outer loop retires
        // this charge only after that scope completes, so its frame stays intact.
        drop(self.occurrence.take());
        Ok(())
    }
}

/// Only the dispatcher can borrow this guard from its actual live session.
/// The journal layer may revalidate it but cannot construct substitute custody.
pub(super) struct PublicationGuard<'a, 'work> {
    admission: &'a Admission<'a>,
    occurrence: Option<&'a NativeOccurrence>,
    context: &'a OccurrenceContext<'work>,
}
impl PublicationGuard<'_, '_> {
    pub fn validate(&self, b: &mut Budget<'_>) -> Result<()> {
        self.admission.validate_continuity(b)?;
        if let Some(occurrence) = self.occurrence {
            self.context.validate(self.admission, occurrence, b)?;
        }
        Ok(())
    }
}

// Identity comparison is not an authority constructor. Callers must separately
// retain and revalidate the actual NativeOccurrence before exposing pending state.
fn matches_pending(record: &Record, subject: &Subject, identity: &[u8; 32]) -> Result<()> {
    let expected = match &record.body {
        Body::Prepared { subject, .. } => subject,
        Body::Issued { request, .. } => request.subject(),
        Body::Ready => return Err(Error::rejected("native journal has no pending occurrence")),
    };
    if identity != &record.occurrence || subject.canonical_bytes() != expected.canonical_bytes() {
        return Err(Error::rejected(
            "native journal changed the retained occurrence",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "compiler_execution_issuer_native_session_tests.rs"]
mod tests;

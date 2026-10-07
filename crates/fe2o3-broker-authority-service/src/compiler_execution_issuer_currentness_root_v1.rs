//! Reduced role constructed only by the original-root authenticated gate.
use super::*;
use fe2o3_runtime_protocol::NativeApplicationCurrentnessRootErrorV1 as CurrentError;

pub(in super::super) struct CurrentnessRoot<'work> {
    endpoint: RootEndpoint<'work>,
    gate: AuthenticatedCurrentnessGate,
    retained: usize,
}
impl<'work> CurrentnessRoot<'work> {
    pub(in super::super) fn from_authenticated(
        endpoint: RootEndpoint<'work>,
        gate: AuthenticatedCurrentnessGate,
        b: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        let retained = RootEndpoint::STORAGE + CurrentRecord::STORAGE + size_of::<Self>();
        b.with_prepaid_scope(RootEndpoint::STORAGE, ENTRY, CHECK_WORK, CHECK_FRAME, |b| {
            endpoint.revalidate(gate.deadline, b)?;
            Ok((
                Self {
                    endpoint,
                    gate,
                    retained,
                },
                retained - RootEndpoint::STORAGE,
            ))
        })
    }

    pub(in super::super) const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(in super::super) fn require_carriage(&self, identity: &[u8; 32]) -> Result<()> {
        if &self.gate.request.carriage_identity() != identity {
            return Err(Error::rejected(
                "currentness request changed registered carriage",
            ));
        }
        Ok(())
    }

    pub(in super::super) fn validate(
        &self,
        admission: &Admission<'_>,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        admission.validate_continuity(b)?;
        self.endpoint.revalidate(self.gate.deadline, b)?;
        if self.gate.request.kind() != CurrentKind::Request {
            return Err(Error::rejected("currentness root role changed"));
        }
        Ok(())
    }
}
pub(super) fn codec(error: CurrentError) -> Error {
    match error {
        CurrentError::Resource(error) => error.into(),
        _ => Error::rejected("currentness root framing rejected"),
    }
}

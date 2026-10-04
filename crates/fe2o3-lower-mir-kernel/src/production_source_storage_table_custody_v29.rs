// Table-only extraction of the original lease failure state. No active-root
// constructor or reference-plan adapter exists at this ownership stage.
impl SourceStorageLayoutsV29<'_> {
    pub(super) fn permits_table_refund(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        bytes: usize,
        budget: &Budget<'_>,
    ) -> bool {
        std::ptr::eq(self.owner, owner)
            && self.lease.custody(budget).is_ok()
            && self
                .lease
                .floor
                .checked_add(self.lease.owned.get())
                .is_some_and(|floor| {
                    budget
                        .storage()
                        .checked_sub(bytes)
                        .is_some_and(|after| after >= floor)
                })
    }
}

#[derive(Clone, Copy, Debug)]
enum SourceStorageFailureObservationV29 {
    Resource(ArgumentResourceV1),
    Unsupported {
        function: u32,
        block: Option<u32>,
        statement: Option<u32>,
        detail: &'static str,
    },
    OwnedDiagnostic,
}

impl SourceStorageFailureObservationV29 {
    fn from_error(error: &Error) -> Self {
        match error {
            Error::ArgumentCorrespondenceResource(resource) => Self::Resource(*resource),
            Error::Unsupported {
                function,
                block,
                statement,
                detail,
            } => Self::Unsupported {
                function: *function,
                block: *block,
                statement: *statement,
                detail: *detail,
            },
            _ => Self::OwnedDiagnostic,
        }
    }

    fn error(self) -> Error {
        match self {
            Self::Resource(resource) => resource.into(),
            Self::Unsupported {
                function,
                block,
                statement,
                detail,
            } => Error::Unsupported {
                function,
                block,
                statement,
                detail,
            },
            Self::OwnedDiagnostic => {
                error("source storage root is poisoned by an earlier owned diagnostic")
            }
        }
    }
}

#[derive(Debug)]
enum SourceStorageFirstErrorV29 {
    Clear,
    Owned(Error),
    Returned(SourceStorageFailureObservationV29),
}

#[derive(Debug)]
struct SourceStorageFailureCellV29(RefCell<SourceStorageFirstErrorV29>);

impl SourceStorageFailureCellV29 {
    fn new() -> Self {
        Self(RefCell::new(SourceStorageFirstErrorV29::Clear))
    }

    fn observation(&self) -> Option<SourceStorageFailureObservationV29> {
        match &*self.0.borrow() {
            SourceStorageFirstErrorV29::Clear => None,
            SourceStorageFirstErrorV29::Owned(error) => {
                Some(SourceStorageFailureObservationV29::from_error(error))
            }
            SourceStorageFirstErrorV29::Returned(observation) => Some(*observation),
        }
    }

    fn first_error(&self) -> Option<Error> {
        self.observation()
            .map(SourceStorageFailureObservationV29::error)
    }

    fn resource(&self) -> Option<ArgumentResourceV1> {
        match self.observation() {
            Some(SourceStorageFailureObservationV29::Resource(resource)) => Some(resource),
            _ => None,
        }
    }

    fn record(&self, error: Error) {
        let mut first = self.0.borrow_mut();
        if matches!(*first, SourceStorageFirstErrorV29::Clear) {
            *first = SourceStorageFirstErrorV29::Owned(error);
        }
    }

    fn into_first(self) -> Option<Error> {
        match self.0.into_inner() {
            SourceStorageFirstErrorV29::Clear => None,
            SourceStorageFirstErrorV29::Owned(error) => Some(error),
            SourceStorageFirstErrorV29::Returned(observation) => Some(observation.error()),
        }
    }

    // Called only by consuming root settlement, never by a query or a handle.
    // The original diagnostic moves once; the permanent poison cannot be reset.
    fn return_first_from_consuming_scope(&self) -> Option<Error> {
        let mut first = self.0.borrow_mut();
        let observation = match &*first {
            SourceStorageFirstErrorV29::Clear => return None,
            SourceStorageFirstErrorV29::Owned(error) => {
                SourceStorageFailureObservationV29::from_error(error)
            }
            SourceStorageFirstErrorV29::Returned(observation) => {
                return Some(observation.error());
            }
        };
        match std::mem::replace(
            &mut *first,
            SourceStorageFirstErrorV29::Returned(observation),
        ) {
            SourceStorageFirstErrorV29::Owned(error) => Some(error),
            SourceStorageFirstErrorV29::Clear => None,
            SourceStorageFirstErrorV29::Returned(prior) => Some(prior.error()),
        }
    }
}

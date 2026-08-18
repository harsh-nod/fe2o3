/// Independent issue #135 property families.
///
/// No variant implies any other variant. In particular, progress and
/// quiescence are deliberately distinct.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ServicePropertyV1 {
    QueueSafe = 1,
    QueueLinearizable = 2,
    TaskAccounted = 3,
    DependencyOrdered = 4,
    PhaseRefined = 5,
    QuiescenceSafe = 6,
    CancellationSafe = 7,
    ServiceProgress = 8,
}

pub const SERVICE_PROPERTY_COUNT_V1: usize = 8;
pub const ALL_SERVICE_PROPERTIES_V1: [ServicePropertyV1; SERVICE_PROPERTY_COUNT_V1] = [
    ServicePropertyV1::QueueSafe,
    ServicePropertyV1::QueueLinearizable,
    ServicePropertyV1::TaskAccounted,
    ServicePropertyV1::DependencyOrdered,
    ServicePropertyV1::PhaseRefined,
    ServicePropertyV1::QuiescenceSafe,
    ServicePropertyV1::CancellationSafe,
    ServicePropertyV1::ServiceProgress,
];

/// Evidence classification inherited from issue #134.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EvidenceStatusV1 {
    Unsupported = 1,
    Contracted = 2,
    Checked = 3,
    Validated = 4,
    Proved = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertyClaimV1 {
    pub property: ServicePropertyV1,
    pub status: EvidenceStatusV1,
}

/// Fixed-width property matrix with no implication or promotion operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyClaimsV1 {
    claims: [PropertyClaimV1; SERVICE_PROPERTY_COUNT_V1],
}

impl Default for PropertyClaimsV1 {
    fn default() -> Self {
        Self::unsupported()
    }
}

impl PropertyClaimsV1 {
    pub const fn unsupported() -> Self {
        Self {
            claims: [
                PropertyClaimV1 {
                    property: ServicePropertyV1::QueueSafe,
                    status: EvidenceStatusV1::Unsupported,
                },
                PropertyClaimV1 {
                    property: ServicePropertyV1::QueueLinearizable,
                    status: EvidenceStatusV1::Unsupported,
                },
                PropertyClaimV1 {
                    property: ServicePropertyV1::TaskAccounted,
                    status: EvidenceStatusV1::Unsupported,
                },
                PropertyClaimV1 {
                    property: ServicePropertyV1::DependencyOrdered,
                    status: EvidenceStatusV1::Unsupported,
                },
                PropertyClaimV1 {
                    property: ServicePropertyV1::PhaseRefined,
                    status: EvidenceStatusV1::Unsupported,
                },
                PropertyClaimV1 {
                    property: ServicePropertyV1::QuiescenceSafe,
                    status: EvidenceStatusV1::Unsupported,
                },
                PropertyClaimV1 {
                    property: ServicePropertyV1::CancellationSafe,
                    status: EvidenceStatusV1::Unsupported,
                },
                PropertyClaimV1 {
                    property: ServicePropertyV1::ServiceProgress,
                    status: EvidenceStatusV1::Unsupported,
                },
            ],
        }
    }

    pub fn get(&self, property: ServicePropertyV1) -> EvidenceStatusV1 {
        self.claims[property_index(property)].status
    }

    /// Replaces exactly one classification and performs no inference.
    pub fn set(&mut self, property: ServicePropertyV1, status: EvidenceStatusV1) {
        self.claims[property_index(property)].status = status;
    }

    pub const fn as_array(&self) -> &[PropertyClaimV1; SERVICE_PROPERTY_COUNT_V1] {
        &self.claims
    }
}

const fn property_index(property: ServicePropertyV1) -> usize {
    property as usize - 1
}

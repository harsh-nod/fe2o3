/// Typed failure from metered exact canonical Kernel IR V12 construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MeteredVerifiedCanonicalKernelIrErrorV12 {
    /// Canonical encoding, decoding, verification, or custody failed.
    Canonical(VerifiedCanonicalKernelIrErrorV12),
    /// The shared canonical work limit rejected the next operation.
    WorkLimit(CanonicalKernelIrWorkLimitV1),
    /// Semantic verification rejected the exact decoded module. The receipt
    /// reports verifier resources observed before transferring diagnostics.
    Verification {
        error: VerificationErrors,
        receipt: CanonicalKernelIrVerificationResourceReceiptV1,
    },
    /// Shared work was exhausted after semantic verification began.
    VerificationWorkLimit {
        error: CanonicalKernelIrWorkLimitV1,
        receipt: CanonicalKernelIrVerificationResourceReceiptV1,
    },
    /// Verifier-local storage, allocation, or accounting failed.
    VerificationResource {
        error: CanonicalKernelIrVerificationResourceErrorV1,
        receipt: CanonicalKernelIrVerificationResourceReceiptV1,
    },
    /// Canonical custody failed after semantic verification completed.
    PostVerificationCanonical {
        error: VerifiedCanonicalKernelIrErrorV12,
        receipt: CanonicalKernelIrVerificationResourceReceiptV1,
    },
}

impl MeteredVerifiedCanonicalKernelIrErrorV12 {
    /// Returns verifier resources observed by failures occurring after the
    /// exact decoded semantic verifier began.
    pub const fn verification_receipt(
        &self,
    ) -> Option<CanonicalKernelIrVerificationResourceReceiptV1> {
        match self {
            Self::Canonical(_) | Self::WorkLimit(_) => None,
            Self::Verification { receipt, .. }
            | Self::VerificationWorkLimit { receipt, .. }
            | Self::VerificationResource { receipt, .. }
            | Self::PostVerificationCanonical { receipt, .. } => Some(*receipt),
        }
    }
}

impl fmt::Display for MeteredVerifiedCanonicalKernelIrErrorV12 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonical(error) => error.fmt(formatter),
            Self::WorkLimit(error) => error.fmt(formatter),
            Self::Verification { error, .. } => error.fmt(formatter),
            Self::VerificationWorkLimit { error, .. } => error.fmt(formatter),
            Self::VerificationResource { error, .. } => error.fmt(formatter),
            Self::PostVerificationCanonical { error, .. } => error.fmt(formatter),
        }
    }
}

impl Error for MeteredVerifiedCanonicalKernelIrErrorV12 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Canonical(error) => Some(error),
            Self::WorkLimit(error) => Some(error),
            Self::Verification { error, .. } => Some(error),
            Self::VerificationWorkLimit { error, .. } => Some(error),
            Self::VerificationResource { error, .. } => Some(error),
            Self::PostVerificationCanonical { error, .. } => Some(error),
        }
    }
}

fn metered_encode_error_v12(
    error: KernelIrEncodeError,
) -> MeteredVerifiedCanonicalKernelIrErrorV12 {
    match error {
        KernelIrEncodeError::WorkLimit(error) => {
            MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(error)
        }
        error => MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
            VerifiedCanonicalKernelIrErrorV12::Encode(error),
        ),
    }
}

fn metered_decode_error_v12(
    error: KernelIrDecodeError,
) -> MeteredVerifiedCanonicalKernelIrErrorV12 {
    match error {
        KernelIrDecodeError::WorkLimit(error)
        | KernelIrDecodeError::Encode(KernelIrEncodeError::WorkLimit(error)) => {
            MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(error)
        }
        error => MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
            VerifiedCanonicalKernelIrErrorV12::Decode(error),
        ),
    }
}

fn metered_post_verification_error_v12(
    error: MeteredVerifiedCanonicalKernelIrErrorV12,
    receipt: CanonicalKernelIrVerificationResourceReceiptV1,
) -> MeteredVerifiedCanonicalKernelIrErrorV12 {
    match error {
        MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(error)
        | MeteredVerifiedCanonicalKernelIrErrorV12::PostVerificationCanonical { error, .. } => {
            MeteredVerifiedCanonicalKernelIrErrorV12::PostVerificationCanonical { error, receipt }
        }
        MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(error)
        | MeteredVerifiedCanonicalKernelIrErrorV12::VerificationWorkLimit { error, .. } => {
            MeteredVerifiedCanonicalKernelIrErrorV12::VerificationWorkLimit { error, receipt }
        }
        MeteredVerifiedCanonicalKernelIrErrorV12::Verification { error, .. } => {
            MeteredVerifiedCanonicalKernelIrErrorV12::Verification { error, receipt }
        }
        MeteredVerifiedCanonicalKernelIrErrorV12::VerificationResource { error, .. } => {
            MeteredVerifiedCanonicalKernelIrErrorV12::VerificationResource { error, receipt }
        }
    }
}

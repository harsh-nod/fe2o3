//! Fixed-width, domain-specific identity commitments.

/// Width of every V1 compiler API identity commitment.
pub const IDENTITY_BYTES_V1: usize = 32;

macro_rules! fixed_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name([u8; IDENTITY_BYTES_V1]);

        impl $name {
            /// Wraps an untrusted fixed-width commitment without authenticating it.
            pub const fn from_untrusted_bytes(bytes: [u8; IDENTITY_BYTES_V1]) -> Self {
                Self(bytes)
            }

            /// Borrows the opaque commitment bytes.
            pub const fn as_bytes(&self) -> &[u8; IDENTITY_BYTES_V1] {
                &self.0
            }

            /// Returns the opaque commitment bytes.
            pub const fn into_bytes(self) -> [u8; IDENTITY_BYTES_V1] {
                self.0
            }
        }
    };
}

fixed_identity!(
    /// Commitment to the complete canonical compile request.
    RequestIdentityV1
);
fixed_identity!(
    /// Commitment to one concrete, authenticated kernel instance.
    KernelInstanceIdentityV1
);
fixed_identity!(
    /// Commitment to the compiler and frontend semantic profile.
    CompilerProfileIdentityV1
);
fixed_identity!(
    /// Commitment to the target capabilities and target selection profile.
    TargetProfileIdentityV1
);
fixed_identity!(
    /// Commitment to the complete selected pipeline configuration.
    PipelineConfigurationIdentityV1
);
fixed_identity!(
    /// Commitment to one canonical stage snapshot.
    SnapshotIdentityV1
);
fixed_identity!(
    /// Commitment to the schema and canonical encoding of a stage snapshot.
    SnapshotFormatIdentityV1
);
fixed_identity!(
    /// Commitment to a transformation implementation and semantic version.
    TransformIdentityV1
);
fixed_identity!(
    /// Commitment to one transformation's complete configuration.
    TransformConfigurationIdentityV1
);
fixed_identity!(
    /// Commitment to a canonical set of proof or refinement obligations.
    ObligationSetIdentityV1
);
fixed_identity!(
    /// Commitment to the semantic entity named by a diagnostic.
    DiagnosticSubjectIdentityV1
);
fixed_identity!(
    /// Commitment to an opaque executable candidate.
    CandidateIdentityV1
);
fixed_identity!(
    /// Commitment to the format and schema of an executable candidate.
    CandidateFormatIdentityV1
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_are_fixed_width_and_preserve_all_bytes() {
        let bytes = [0xa5; IDENTITY_BYTES_V1];
        let identity = SnapshotIdentityV1::from_untrusted_bytes(bytes);

        assert_eq!(identity.as_bytes(), &bytes);
        assert_eq!(identity.into_bytes(), bytes);
        assert_eq!(
            core::mem::size_of::<SnapshotIdentityV1>(),
            IDENTITY_BYTES_V1
        );
    }

    #[test]
    fn zero_commitments_are_shape_valid_but_not_authenticated() {
        let identity = RequestIdentityV1::from_untrusted_bytes([0; IDENTITY_BYTES_V1]);
        assert_eq!(identity.as_bytes(), &[0; IDENTITY_BYTES_V1]);
    }
}

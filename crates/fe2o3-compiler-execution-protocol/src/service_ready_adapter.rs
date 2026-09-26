//! Closed native service ready body over the unchanged V1 wire.
macro_rules! service_ready_adapter {
    ($bytes:ident, $work:ident, $storage:ident, $Owner:ident, $Failure:ident) => {
        /// Native API size; the identity-only readiness wire remains version 1.
        pub const $bytes: usize = codec::BYTES;
        const RETAINED: usize = size_of::<($Owner, Storage)>();
        /// Logical work for entry, fixed framing, hashing, reencoding and comparison.
        /// This is not an instruction or wall-time bound.
        pub const $work: usize =
            resources::ENTRY_WORK + 32 * codec::BYTES;
        /// Additional logical scratch. Borrowed inputs remain separately prepaid.
        /// This is not an allocator, generated-stack or RSS bound.
        pub const $storage: usize =
            4 * RETAINED + 4 * codec::BYTES + 2 * size_of::<sha2::Sha256>() + 4096;

        const _: () = {
            assert!(
                8 * size_of::<$Failure>() + 64 * size_of::<usize>() <= 4096
            );
        };

        impl $Owner {
            pub fn new(
                issuer_pid: u32,
                launch: &Manifest,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                metered(
                    budget,
                    launch.retained_storage() + policy.retained_storage(),
                    || {
                        if issuer_pid == 0 {
                            return Err(Framing::IssuerPid.into());
                        }
                        if launch.policy_identity() != policy.identity() {
                            return Err(Framing::PolicyMismatch.into());
                        }
                        Ok((
                            Self {
                                record: codec::encode(
                                    issuer_pid,
                                    *launch.identity().as_bytes(),
                                    *policy.identity().as_bytes(),
                                ),
                            },
                            Storage(RETAINED),
                        ))
                    },
                )
            }

            /// Strict shared-wire decode and independent reencode. Native contextual
            /// matching and protected readiness admission remain caller obligations.
            pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                metered(
                    budget,
                    resources::fixed_input_floor(bytes, codec::BYTES),
                    || {
                        Ok((
                            Self {
                                record: codec::decode(bytes)?,
                            },
                            Storage(RETAINED),
                        ))
                    },
                )
            }

            /// Requires this PID, both exact identities and the native manifest's
            /// binding to the supplied policy. Even a mismatch is prepaid in full.
            pub fn matches_launch(
                &self,
                issuer_pid: u32,
                launch: &Manifest,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<bool> {
                metered(
                    budget,
                    RETAINED + launch.retained_storage() + policy.retained_storage(),
                    || {
                        Ok(self.record.issuer_pid == issuer_pid
                            && &self.record.launch_manifest == launch.identity().as_bytes()
                            && &self.record.policy == policy.identity().as_bytes()
                            && launch.policy_identity() == policy.identity())
                    },
                )
            }

            pub const fn issuer_pid(&self) -> u32 {
                self.record.issuer_pid
            }
            pub const fn launch_manifest_identity(&self) -> ManifestIdentity {
                ManifestIdentity::from_bytes_for_protocol(self.record.launch_manifest)
            }
            /// Opaque identity interpreted by this native API; not native policy admission.
            pub const fn policy_identity(&self) -> PolicyIdentity {
                PolicyIdentity::from_bytes_for_protocol(self.record.policy)
            }
            /// Wire identity retains the existing framing domain and identity type.
            pub const fn identity(&self) -> Identity {
                Identity::from_bytes_for_protocol(self.record.identity)
            }
            pub const fn canonical_bytes(&self) -> &[u8; $bytes] {
                &self.record.bytes
            }
            pub const fn retained_storage(&self) -> usize {
                RETAINED
            }
        }

        impl fmt::Debug for $Owner {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Owner))
                    .field("authority", &"none")
                    .field("issuer_pid", &self.issuer_pid())
                    .field("identity", &self.identity())
                    .finish_non_exhaustive()
            }
        }

        /// Bounded framing and resource diagnostics; no V1 owner admission.
        #[derive(Debug)]
        pub enum $Failure {
            Framing(Framing),
            Resource(Resource),
        }
        type Result<T> = std::result::Result<T, $Failure>;
        impl From<Framing> for $Failure {
            fn from(value: Framing) -> Self {
                Self::Framing(value)
            }
        }
        impl From<Resource> for $Failure {
            fn from(value: Resource) -> Self {
                Self::Resource(value)
            }
        }
        impl fmt::Display for $Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Framing(e) => e.fmt(f),
                    Self::Resource(e) => e.fmt(f),
                }
            }
        }
        impl Error for $Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(match self {
                    Self::Framing(e) => e,
                    Self::Resource(e) => e,
                })
            }
        }

        fn metered<T>(
            budget: &mut Budget<'_>,
            floor: usize,
            operation: impl FnOnce() -> Result<T>,
        ) -> Result<T> {
            resources::fixed(
                budget,
                floor,
                $work,
                $storage,
                operation,
            )
        }
    };
}
pub(crate) use service_ready_adapter;

//! Closed native launch manifest body over the unchanged V1 wire.
macro_rules! launch_manifest_adapter {
    ($bytes:ident, $work:ident, $storage:ident, $Owner:ident, $Failure:ident) => {
        /// Native API size; the identity-only wire remains version 1.
        pub const $bytes: usize = codec::BYTES;
        const RETAINED: usize = size_of::<($Owner, Storage)>();
        /// Logical work for fixed framing, encoding, hashing and comparison, not instructions.
        pub const $work: usize =
            resources::ENTRY_WORK + 32 * codec::BYTES;
        /// Additional logical scratch. Inputs remain separately prepaid. Not an RSS,
        /// allocator, generated-stack or wall-time bound.
        pub const $storage: usize =
            4 * RETAINED + 4 * codec::BYTES + 2 * size_of::<sha2::Sha256>() + 4096;

        const _: () = {
            assert!(
                8 * size_of::<$Failure>() + 64 * size_of::<usize>()
                    <= 4096
            );
        };

        impl $Owner {
            // Used only by prepaid shared-codec adapters, never an admitted V1 projection.
            pub(crate) fn from_record(record: codec::Record) -> Self {
                Self { record }
            }

            pub fn new(
                client: Client,
                service: Service,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                metered(budget, policy.retained_storage(), || {
                    Ok((
                        Self {
                            record: codec::encode(client, service, *policy.identity().as_bytes()),
                        },
                        Storage(RETAINED),
                    ))
                })
            }

            /// Strict shared-wire framing only. A native contextual policy match is still required.
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

            pub fn matches_policy(&self, policy: &Policy, budget: &mut Budget<'_>) -> Result<bool> {
                metered(budget, RETAINED + policy.retained_storage(), || {
                    Ok(&self.record.policy == policy.identity().as_bytes())
                })
            }

            pub const fn client(&self) -> Client {
                self.record.client
            }
            pub const fn external_anchor_service(&self) -> Service {
                self.record.service
            }
            /// Opaque identity interpreted by this native API; not native policy admission.
            pub const fn policy_identity(&self) -> PolicyIdentity {
                PolicyIdentity::from_bytes_for_protocol(self.record.policy)
            }
            /// Wire identity keeps the existing framing domain and identity type.
            pub const fn identity(&self) -> Identity {
                Identity::from_bytes_for_protocol(self.record.identity)
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
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
                    .field("client", &self.client())
                    .field("identity", &self.identity())
                    .finish_non_exhaustive()
            }
        }

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
pub(crate) use launch_manifest_adapter;

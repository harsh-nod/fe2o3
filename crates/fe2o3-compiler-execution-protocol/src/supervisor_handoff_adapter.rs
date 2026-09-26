//! Closed native supervisor handoff body over the unchanged V1 wire.
macro_rules! supervisor_handoff_adapter {
    ($bytes:ident, $work:ident, $storage:ident, $Owner:ident, $Failure:ident) => {
        /// Native API size; the 184-byte identity-only wire and hash domain remain V1.
        pub const $bytes: usize = codec::BYTES;
        const RETAINED: usize = size_of::<($Owner, Storage)>();
        /// Complete logical quota for both handoff and nested launch framing, hashing,
        /// re-encoding and comparison: 9,480 units. One entry charge, no nested budget.
        /// This is not a bound on generated instructions or elapsed time.
        pub const $work: usize =
            resources::ENTRY_WORK + 32 * (codec::BYTES + launch_manifest_codec::BYTES);
        /// Additional fixed scratch above all prepaid live inputs. Covers owner/record
        /// moves, both fixed wires, SHA state and guarded scalar/result controls. Not an
        /// allocator, generated-stack or process-RSS bound.
        pub const $storage: usize = 4 * RETAINED
            + 4 * (codec::BYTES + launch_manifest_codec::BYTES)
            + 2 * size_of::<sha2::Sha256>()
            + 4096;

        const _: () = {
            type Output = ($Owner, Storage);
            type Raw = (codec::Frame, launch_manifest_codec::Record);
            assert!(codec::BYTES == 184 && launch_manifest_codec::BYTES == 112);
            assert!(RETAINED >= size_of::<(Launch, Storage)>());
            assert!(size_of::<Raw>() <= RETAINED);
            assert!(
                size_of::<codec::Frame>() + size_of::<launch_manifest_codec::Record>() <= RETAINED
            );
            assert!(size_of::<Result<Output>>() >= RETAINED);
            assert!(size_of::<std::thread::Result<Result<Output>>>() >= RETAINED);
            assert!(size_of::<std::result::Result<Raw, Framing>>() >= size_of::<Raw>());
            assert!(
                size_of::<std::result::Result<codec::Frame, Framing>>()
                    >= size_of::<codec::Frame>()
            );
            assert!(
                8 * size_of::<$Failure>()
                    + 64 * size_of::<usize>()
                    + size_of::<Budget<'static>>()
                    + 4 * size_of::<Storage>()
                    + (size_of::<Result<Output>>() - RETAINED)
                    + (size_of::<std::thread::Result<Result<Output>>>() - RETAINED)
                    + (size_of::<std::result::Result<Raw, Framing>>() - size_of::<Raw>())
                    + (size_of::<std::result::Result<codec::Frame, Framing>>()
                        - size_of::<codec::Frame>())
                    <= 4096
            );
        };

        impl $Owner {
            /// Consumes a fully prepaid native launch owner. Keep its reservation and
            /// add only the returned growth; eventual release uses `retained_storage()`.
            pub fn new(
                submitter: Client,
                launch: Launch,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                let inherited = launch.retained_storage();
                metered(budget, inherited, || {
                    let growth = RETAINED
                        .checked_sub(inherited)
                        .ok_or(Resource::Accounting)?;
                    let frame =
                        codec::encode(submitter, launch.client(), launch.canonical_bytes())?;
                    Ok((
                        Self {
                            frame,
                            launch_manifest: launch,
                        },
                        Storage(growth),
                    ))
                })
            }

            /// Strict shared framing with full retained storage returned unreserved.
            /// Exactly sized input needs all 184 bytes prepaid; wrong lengths need no
            /// input floor. The complete fixed quota includes the private launch codec.
            pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                metered(
                    budget,
                    resources::fixed_input_floor(bytes, codec::BYTES),
                    || {
                        let (frame, launch) = codec::decode(bytes)?;
                        Ok((
                            Self {
                                frame,
                                launch_manifest: Launch::from_record(launch),
                            },
                            Storage(RETAINED),
                        ))
                    },
                )
            }

            pub const fn submitter(&self) -> Client {
                self.frame.submitter
            }
            pub const fn launch_manifest(&self) -> &Launch {
                &self.launch_manifest
            }
            /// Existing wire identity, not evidence of native policy admission.
            pub const fn identity(&self) -> Identity {
                Identity::from_bytes_for_protocol(self.frame.identity)
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
                &self.frame.bytes
            }
            /// Full owner and padded storage-receipt header, including the nested launch.
            pub const fn retained_storage(&self) -> usize {
                RETAINED
            }
        }

        impl fmt::Debug for $Owner {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Owner))
                    .field("authority", &"none")
                    .field("submitter", &self.submitter())
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
            resources::fixed(budget, floor, $work, $storage, operation)
        }
    };
}
pub(crate) use supervisor_handoff_adapter;

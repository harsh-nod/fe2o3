//! Shared physical finalization with concrete, non-interchangeable wire owners.

macro_rules! mixed_descriptor_finalization_family {
    (
        $section_name:literal, $section:ident, $digest_offset:ident,
        $reader_storage:ident, $scratch:ident, $table:ident, $wire_error:ident,
        $decode:ident, $inspection:ident, $inspection_docs:literal,
        $finalized:ident, $owner_docs:literal, $error:ident,
        $inspect_raw:ident, $inspect_final:ident, $finalize:ident, $derive:ident
    ) => {
        use std::{error::Error, fmt, mem::size_of};

        use fe2o3_hsaco::InspectedKernelBindings;
        use fe2o3_kernel_descriptor::{
            $digest_offset, $section,
            CanonicalCodeObjectDigest, DESCRIPTOR_QUERY_STORAGE_V3, DescriptorWireErrorV3,
            $reader_storage, $wire_error, $table,
            $decode,
        };
        use sha2::Sha256;

        use crate::{
            DescriptorSectionLocation, FinalizationError,
            nominal_descriptor_common::{self as common, Failure, Format, Inspection},
            nominal_descriptor_physical::cross_check,
        };

        /// Simultaneous descriptor view, mandatory-contract reader/query and hash scratch.
        /// Excludes caller bytes, callback state, returned owner and ELF/AMDHSA allocations.
        /// This is an inert storage declaration, not an authenticated reservation.
        /// Shared inspection/format headers and physical projection/launch copies are
        /// included as logical typed extents, not compiler stack or whole-process usage.
        pub const $scratch: usize = $reader_storage
            + DESCRIPTOR_QUERY_STORAGE_V3
            + size_of::<Sha256>()
            + size_of::<$inspection<'static>>()
            + common::COMMON_STORAGE
            + crate::nominal_descriptor_physical::PHYSICAL_PROJECTION_STORAGE
            + 256;

        #[derive(Debug)]
        pub enum $error<E> {
            Artifact(FinalizationError),
            Wire($wire_error<E>),
            Work(E),
            Scratch { required: usize, prepaid: usize },
            DescriptorSourceMismatch,
        }
        impl<E: fmt::Display> fmt::Display for $error<E> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Artifact(FinalizationError::MissingDescriptorSection) => {
                        f.write_str(concat!("ELF section ", $section_name, " is missing"))
                    }
                    Self::Artifact(FinalizationError::DuplicateDescriptorSection) => {
                        f.write_str(concat!("ELF contains multiple ", $section_name, " sections"))
                    }
                    Self::Artifact(e) => write!(f, "conditional nominal HSACO inspection failed: {e}"),
                    Self::Wire(e) => write!(f, "conditional nominal descriptor rejected: {e}"),
                    Self::Work(e) => write!(f, "conditional nominal descriptor work refused: {e}"),
                    Self::Scratch { required, prepaid } => write!(
                        f, "conditional nominal descriptor needs {required} scratch bytes, got {prepaid}"
                    ),
                    Self::DescriptorSourceMismatch => f.write_str(
                        "embedded conditional nominal descriptor differs from exact zero-digest compiler source",
                    ),
                }
            }
        }
        impl<E: Error + 'static> Error for $error<E> {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Artifact(e) => Some(e),
                    Self::Wire(e) => Some(e),
                    Self::Work(e) => Some(e),
                    _ => None,
                }
            }
        }
        impl<E> From<FinalizationError> for $error<E> {
            fn from(e: FinalizationError) -> Self {
                Self::Artifact(e)
            }
        }
        impl<E> From<$wire_error<E>> for $error<E> {
            fn from(e: $wire_error<E>) -> Self {
                Self::Wire(e)
            }
        }
        impl<E> From<DescriptorWireErrorV3<E>> for $error<E> {
            fn from(e: DescriptorWireErrorV3<E>) -> Self {
                Self::Wire(e.into())
            }
        }
        impl<E> Failure<E> for $error<E> {
            fn work(error: E) -> Self {
                Self::Work(error)
            }
            fn scratch(required: usize, prepaid: usize) -> Self {
                Self::Scratch { required, prepaid }
            }
            fn source_mismatch() -> Self {
                Self::DescriptorSourceMismatch
            }
        }
        type FamilyResult<T, E> = Result<T, $error<E>>;

        #[doc = $inspection_docs]
        pub struct $inspection<'a> {
            bindings: InspectedKernelBindings,
            table: $table<'a>,
            location: DescriptorSectionLocation,
        }
        impl fmt::Debug for $inspection<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($inspection))
                    .field("location", &self.location)
                    .field("bindings", &self.bindings)
                    .finish_non_exhaustive()
            }
        }
        impl<'a> $inspection<'a> {
            pub fn descriptor_table(&self) -> &$table<'a> {
                &self.table
            }
            pub fn kernel_bindings(&self) -> &InspectedKernelBindings {
                &self.bindings
            }
            /// Transfers physical metadata only; the returned metadata remains inert.
            pub fn into_kernel_bindings(self) -> InspectedKernelBindings {
                self.bindings
            }
            pub fn location(&self) -> DescriptorSectionLocation {
                self.location
            }
            pub fn digest(&self) -> CanonicalCodeObjectDigest {
                self.table.canonical_code_object_digest()
            }
            pub const fn grants_launch_authority(&self) -> bool {
                false
            }
            fn into_inspection(self) -> Inspection {
                Inspection {
                    digest: self.digest(),
                    bindings: self.bindings,
                    location: self.location,
                }
            }
        }

        #[doc = $owner_docs]
        #[derive(Debug)]
        pub struct $finalized {
            bytes: Vec<u8>,
            bindings: InspectedKernelBindings,
            location: DescriptorSectionLocation,
            digest: CanonicalCodeObjectDigest,
        }
        impl $finalized {
            pub fn as_bytes(&self) -> &[u8] {
                &self.bytes
            }
            pub fn into_bytes(self) -> Vec<u8> {
                self.bytes
            }
            /// Actual byte-buffer capacity only; not a storage receipt or allocation credit.
            pub fn artifact_byte_capacity(&self) -> usize {
                self.bytes.capacity()
            }
            pub fn kernel_bindings(&self) -> &InspectedKernelBindings {
                &self.bindings
            }
            pub fn location(&self) -> DescriptorSectionLocation {
                self.location
            }
            pub fn digest(&self) -> CanonicalCodeObjectDigest {
                self.digest
            }
            pub fn descriptor_bytes(&self) -> &[u8] {
                &self.bytes[self.location.offset..self.location.offset + self.location.size]
            }
            pub const fn grants_launch_authority(&self) -> bool {
                false
            }
        }

        fn inspect<'a, E>(
            bytes: &'a [u8],
            finalized: bool,
            prepaid_scratch: usize,
            charge: &mut impl FnMut(usize) -> Result<(), E>,
        ) -> FamilyResult<$inspection<'a>, E> {
            let (table, inspected) = common::inspect(
                bytes,
                finalized,
                prepaid_scratch,
                Format {
                    section_name: $section,
                    digest_offset: $digest_offset,
                    scratch: $scratch,
                },
                charge,
                |wire, bindings, charge| {
                    let table = $decode(wire, charge)?;
                    cross_check(bindings, &table, charge)?;
                    let digest = table.canonical_code_object_digest();
                    Ok::<_, $error<E>>((table, digest))
                },
            )?;
            Ok($inspection {
                bindings: inspected.bindings,
                table,
                location: inspected.location,
            })
        }

        /// Inspect exactly one detached family-specific section, retaining every contract.
        /// Reject duplicates and every other schema in `.fe2o3.kd.*`, including future
        /// names, without fallback or contract erasure.
        /// COV6 requires a declared explicit-size + 256 ABI; physical metadata and hardware
        /// must agree on explicit-only or complete hidden-tail form, as in nominal V3.
        pub fn $inspect_raw<'a, E>(
            bytes: &'a [u8],
            prepaid_scratch: usize,
            charge: &mut impl FnMut(usize) -> Result<(), E>,
        ) -> FamilyResult<$inspection<'a>, E> {
            inspect(bytes, false, prepaid_scratch, charge)
        }

        /// Independently verify physical metadata, mandatory contracts and artifact digest.
        pub fn $inspect_final<'a, E>(
            bytes: &'a [u8],
            prepaid_scratch: usize,
            charge: &mut impl FnMut(usize) -> Result<(), E>,
        ) -> FamilyResult<$inspection<'a>, E> {
            inspect(bytes, true, prepaid_scratch, charge)
        }

        /// Require exact zero-digest source bytes, patch only the digest, and reinspect.
        /// Caller-supplied source bytes and their hashes cannot supply owned proof or machine evidence.
        pub fn $finalize<E>(
            bytes: &[u8],
            expected_descriptor_source: &[u8],
            prepaid_scratch: usize,
            charge: &mut impl FnMut(usize) -> Result<(), E>,
        ) -> FamilyResult<$finalized, E> {
            let (output, verified) = common::finalize(
                bytes,
                expected_descriptor_source,
                charge,
                |bytes, finalized, charge| {
                    Ok::<_, $error<E>>(
                        inspect(bytes, finalized, prepaid_scratch, charge)?.into_inspection(),
                    )
                },
            )?;
            Ok($finalized {
                bytes: output,
                bindings: verified.bindings,
                location: verified.location,
                digest: verified.digest,
            })
        }

        /// Verify, clear only the digest, and independently validate the raw artifact.
        pub fn $derive<E>(
            bytes: &[u8],
            prepaid_scratch: usize,
            charge: &mut impl FnMut(usize) -> Result<(), E>,
        ) -> FamilyResult<Vec<u8>, E> {
            common::derive_unfinalized(bytes, charge, |bytes, finalized, charge| {
                Ok(inspect(bytes, finalized, prepaid_scratch, charge)?.into_inspection())
            })
        }
    };
}

pub(crate) use mixed_descriptor_finalization_family;

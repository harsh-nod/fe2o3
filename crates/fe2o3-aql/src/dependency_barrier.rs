//! Inert dependency barriers, separate from the zero-dependency liveness probe.
//!
//! Numeric signal observations do not establish mapping, lifetime, topology,
//! ownership, generation, producer completion, or cross-device visibility.
//! A future retained runtime must validate those properties and an acyclic
//! dependency schedule before accepting these bytes. No native queue accepts
//! this packet through the existing zero-dependency publication contract.

use core::mem::{align_of, offset_of, size_of};

use crate::{
    AMD_SIGNAL_ALIGNMENT_V1, AQL_BARRIER_AND_PACKET_BYTES_V1, AQL_INVALID_PACKET_HEADER_V1,
    AqlAddressObservationError, ObservedGpuAddressV1,
};

pub const AQL_MAX_BARRIER_AND_DEPENDENCIES_V1: usize = 5;
pub const AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_DEPENDENCY_BARRIER_AND_HEADER_V1: u16 = 0x1503;

/// Stable name of the separately reviewed inert dependency-packet contract.
pub const AQL_DEPENDENCY_BARRIER_AND_ABI_SCHEMA_ID_V1: &str =
    "rocr-7.2.4-amdhsa-aql-dependency-barrier-and-v1";

/// Source and byte-layout identity, not a native runtime admission contract.
pub const AQL_DEPENDENCY_BARRIER_AND_ABI_SCHEMA_MANIFEST_V1: &str = r#"schema=rocr-7.2.4-amdhsa-aql-dependency-barrier-and-v1
platform=linux-x86_64,little-endian,pointer-width:64
rocr_commit=97f5574fe2fdc7bef44fb01545347912ee9f1779,tag:rocm-7.2.4
source.hsa.h=51ea864cc3e83a9ce824c294dd98a5724eeec87b76fafded1a01d406206ce0f5
dispatch_signal_schema_sha256=82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0
packet=size:64,align:8,header:0,reserved0:2,reserved1:4,dep-signals:8,16,24,32,40,reserved2:48,completion-signal:56
publication=initial-type:invalid-1,all-reserved-bytes-zero,wait-for-prior-system-scope-final-header:0x1503,single-release-u32-at-offset-0,type:3,barrier:1,acquire:system-2,release:system-2
dependencies=count:1..5,nonzero,64-byte-aligned,distinct,in-caller-order,unused-handles-zero
completion=nonzero,64-byte-aligned,distinct-from-every-dependency
missing-runtime-checks=signal-kind-value-generation,mapped-all-participants,retained-owner-and-mapping-lifetimes,topology-currentness,producer-release,consumer-acquire,acyclic-progress,terminal-retention
authority=inert-wire-values-only,no-address-provenance,no-allocation,no-typed-object-placement,no-native-admission,no-queue,no-publication,no-doorbell,no-execution
"#;

/// SHA-256 of [`AQL_DEPENDENCY_BARRIER_AND_ABI_SCHEMA_MANIFEST_V1`].
pub const AQL_DEPENDENCY_BARRIER_AND_ABI_SCHEMA_MANIFEST_SHA256_V1: &str =
    "9cf76f66e74693f0baa8e110b6963d113a6d6501820ac193c179e154f6e02ddf";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AqlDependencyBarrierAndPacketErrorV1 {
    DependencyCount,
    DependencySignal {
        index: usize,
        error: AqlAddressObservationError,
    },
    DuplicateDependency {
        index: usize,
    },
    CompletionSignal(AqlAddressObservationError),
    CompletionAliasesDependency {
        index: usize,
    },
}

/// Exact unpublished 64-byte barrier with one through five dependencies.
///
/// Each handle is only a numeric observation. In particular, alignment is not
/// evidence that a signal is alive, mapped on another device, or from the right
/// producer epoch. This value does not validate a dependency graph or prevent a
/// wait on a future producer. Those checks belong to a retained queue-group owner.
#[derive(Debug, Eq, PartialEq)]
#[repr(C)]
pub struct AqlDependencyBarrierAndPacketV1 {
    full_header: u32,
    reserved1: u32,
    dep_signals: [u64; AQL_MAX_BARRIER_AND_DEPENDENCIES_V1],
    reserved2: u64,
    completion_signal: u64,
}

impl AqlDependencyBarrierAndPacketV1 {
    /// Prepares a fixed system-scoped barrier that also waits for prior packets.
    ///
    /// The narrower policy deliberately rejects duplicate dependencies and a
    /// completion signal that aliases a dependency. Zero-dependency probes use
    /// the unchanged [`crate::AqlBarrierAndPacketV1`] contract instead.
    ///
    /// # Errors
    /// Rejects a count outside one through five, misaligned observations,
    /// duplicate dependencies, or an aliased completion signal.
    pub fn new_unpublished(
        dependencies: &[ObservedGpuAddressV1],
        completion_signal: ObservedGpuAddressV1,
    ) -> Result<AqlPreparedDependencyBarrierAndV1, AqlDependencyBarrierAndPacketErrorV1> {
        if !(1..=AQL_MAX_BARRIER_AND_DEPENDENCIES_V1).contains(&dependencies.len()) {
            return Err(AqlDependencyBarrierAndPacketErrorV1::DependencyCount);
        }
        let completion_signal = completion_signal
            .require_alignment(AMD_SIGNAL_ALIGNMENT_V1 as u64)
            .map_err(AqlDependencyBarrierAndPacketErrorV1::CompletionSignal)?;
        let mut dep_signals = [0; AQL_MAX_BARRIER_AND_DEPENDENCIES_V1];
        for (index, signal) in dependencies.iter().copied().enumerate() {
            let signal = signal
                .require_alignment(AMD_SIGNAL_ALIGNMENT_V1 as u64)
                .map_err(
                    |error| AqlDependencyBarrierAndPacketErrorV1::DependencySignal { index, error },
                )?
                .raw();
            if dep_signals[..index].contains(&signal) {
                return Err(AqlDependencyBarrierAndPacketErrorV1::DuplicateDependency { index });
            }
            if signal == completion_signal.raw() {
                return Err(
                    AqlDependencyBarrierAndPacketErrorV1::CompletionAliasesDependency { index },
                );
            }
            dep_signals[index] = signal;
        }
        Ok(AqlPreparedDependencyBarrierAndV1 {
            packet: Self {
                full_header: u32::from(AQL_INVALID_PACKET_HEADER_V1),
                reserved1: 0,
                dep_signals,
                reserved2: 0,
                completion_signal: completion_signal.raw(),
            },
        })
    }

    pub const fn is_unpublished(&self) -> bool {
        self.full_header == AQL_INVALID_PACKET_HEADER_V1 as u32
    }

    pub const fn dependency_signals(&self) -> [u64; AQL_MAX_BARRIER_AND_DEPENDENCIES_V1] {
        self.dep_signals
    }

    pub const fn completion_signal(&self) -> u64 {
        self.completion_signal
    }

    pub fn encode_unpublished_le(&self) -> [u8; AQL_BARRIER_AND_PACKET_BYTES_V1] {
        let mut bytes = [0; AQL_BARRIER_AND_PACKET_BYTES_V1];
        bytes[..4].copy_from_slice(&self.full_header.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.reserved1.to_le_bytes());
        for (index, signal) in self.dep_signals.iter().enumerate() {
            let offset = 8 + index * size_of::<u64>();
            bytes[offset..offset + size_of::<u64>()].copy_from_slice(&signal.to_le_bytes());
        }
        bytes[48..56].copy_from_slice(&self.reserved2.to_le_bytes());
        bytes[56..64].copy_from_slice(&self.completion_signal.to_le_bytes());
        bytes
    }
}

/// Linear preparation that keeps the dependency body and exact header paired.
///
/// The wrapper cannot be copied or cloned, but it is still an inert value, not
/// an ownership capability. A backend must retain every external signal and
/// mapping through all dependent consumers and uncertain failure paths.
///
/// ```compile_fail
/// fn needs_copy<T: Copy>() {}
/// needs_copy::<fe2o3_aql::AqlPreparedDependencyBarrierAndV1>();
/// ```
///
/// ```compile_fail
/// fn needs_clone<T: Clone>() {}
/// needs_clone::<fe2o3_aql::AqlPreparedDependencyBarrierAndV1>();
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct AqlPreparedDependencyBarrierAndV1 {
    packet: AqlDependencyBarrierAndPacketV1,
}

impl AqlPreparedDependencyBarrierAndV1 {
    /// Passes the invalid body before its fixed release header to the backend.
    ///
    /// # Errors
    /// Returns the first backend error without attempting a later publication.
    pub fn publish_with<T: AqlDependencyBarrierAndPublicationTargetV1>(
        self,
        target: &mut T,
    ) -> Result<(), T::Error> {
        target.write_unpublished_dependency_barrier(&self.packet)?;
        target.publish_dependency_barrier_release_header(
            AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_DEPENDENCY_BARRIER_AND_HEADER_V1,
        )
    }
}

/// Separate backend boundary; legacy zero-dependency backends do not implement it.
///
/// An implementation must establish signal ownership, mappings, visibility,
/// producer/consumer lifetimes, current topology, and an acyclic dependency
/// schedule. Merely implementing this trait grants none of those properties,
/// nor packet-slot, queue, publication, or doorbell authority.
pub trait AqlDependencyBarrierAndPublicationTargetV1 {
    type Error;

    fn write_unpublished_dependency_barrier(
        &mut self,
        packet: &AqlDependencyBarrierAndPacketV1,
    ) -> Result<(), Self::Error>;

    fn publish_dependency_barrier_release_header(&mut self, header: u16)
    -> Result<(), Self::Error>;
}

const _: () = {
    assert!(size_of::<AqlDependencyBarrierAndPacketV1>() == 64);
    assert!(align_of::<AqlDependencyBarrierAndPacketV1>() == 8);
    assert!(offset_of!(AqlDependencyBarrierAndPacketV1, full_header) == 0);
    assert!(offset_of!(AqlDependencyBarrierAndPacketV1, reserved1) == 4);
    assert!(offset_of!(AqlDependencyBarrierAndPacketV1, dep_signals) == 8);
    assert!(offset_of!(AqlDependencyBarrierAndPacketV1, reserved2) == 48);
    assert!(offset_of!(AqlDependencyBarrierAndPacketV1, completion_signal) == 56);
};

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::String, vec::Vec};
    use sha2::{Digest, Sha256};

    fn address(raw: u64) -> ObservedGpuAddressV1 {
        ObservedGpuAddressV1::new(raw).unwrap()
    }

    fn prepared(dependencies: &[u64]) -> AqlPreparedDependencyBarrierAndV1 {
        let dependencies = dependencies
            .iter()
            .copied()
            .map(address)
            .collect::<Vec<_>>();
        AqlDependencyBarrierAndPacketV1::new_unpublished(&dependencies, address(0x9000)).unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut value = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            value.push(char::from(DIGITS[usize::from(*byte >> 4)]));
            value.push(char::from(DIGITS[usize::from(*byte & 0x0f)]));
        }
        value
    }

    #[test]
    fn exact_layout_and_one_dependency_bytes() {
        let value = prepared(&[0x1040]);
        let bytes = value.packet.encode_unpublished_le();
        let mut expected = [0; 64];
        expected[0] = 1;
        expected[8..16].copy_from_slice(&0x1040_u64.to_le_bytes());
        expected[56..64].copy_from_slice(&0x9000_u64.to_le_bytes());
        assert_eq!(bytes, expected);
        assert!(value.packet.is_unpublished());
        assert_eq!(value.packet.dependency_signals(), [0x1040, 0, 0, 0, 0]);
        assert_eq!(value.packet.completion_signal(), 0x9000);
    }

    #[test]
    fn all_five_slots_preserve_caller_order_and_full_width_addresses() {
        let signals = [0xffff_ffff_ffff_ffc0, 0x40, 0x1234_5678_0000, 0x80, 0x1000];
        let value = prepared(&signals);
        assert_eq!(value.packet.dependency_signals(), signals);
        let bytes = value.packet.encode_unpublished_le();
        for (index, signal) in signals.iter().enumerate() {
            assert_eq!(&bytes[8 + index * 8..16 + index * 8], &signal.to_le_bytes());
        }
        assert_eq!(&bytes[2..8], &[0; 6]);
        assert_eq!(&bytes[48..56], &[0; 8]);
    }

    #[test]
    fn unused_dependencies_are_zero_for_each_admitted_count() {
        for count in 1..=5 {
            let signals = [0x40, 0x80, 0xc0, 0x100, 0x140];
            let value = prepared(&signals[..count]);
            assert_eq!(
                &value.packet.dependency_signals()[count..],
                &[0; 5][count..]
            );
        }
    }

    #[test]
    fn dependency_count_is_checked_before_signal_alignment() {
        for dependencies in [Vec::new(), alloc::vec![address(1); 6]] {
            assert_eq!(
                AqlDependencyBarrierAndPacketV1::new_unpublished(&dependencies, address(1)),
                Err(AqlDependencyBarrierAndPacketErrorV1::DependencyCount)
            );
        }
    }

    #[test]
    fn all_signal_positions_require_alignment_and_nonzero_observations() {
        assert_eq!(
            ObservedGpuAddressV1::new(0),
            Err(AqlAddressObservationError::Zero)
        );
        for index in 0..5 {
            let mut signals = [0x40, 0x80, 0xc0, 0x100, 0x140].map(address);
            signals[index] = address(65);
            assert_eq!(
                AqlDependencyBarrierAndPacketV1::new_unpublished(&signals, address(0x9000)),
                Err(AqlDependencyBarrierAndPacketErrorV1::DependencySignal {
                    index,
                    error: AqlAddressObservationError::Misaligned,
                })
            );
        }
        assert_eq!(
            AqlDependencyBarrierAndPacketV1::new_unpublished(&[address(64)], address(65)),
            Err(AqlDependencyBarrierAndPacketErrorV1::CompletionSignal(
                AqlAddressObservationError::Misaligned
            ))
        );
    }

    #[test]
    fn duplicate_and_completion_alias_checks_cover_every_position() {
        for duplicate in 1..5 {
            let mut signals = [0x40, 0x80, 0xc0, 0x100, 0x140].map(address);
            signals[duplicate] = signals[0];
            assert_eq!(
                AqlDependencyBarrierAndPacketV1::new_unpublished(&signals, address(0x9000)),
                Err(AqlDependencyBarrierAndPacketErrorV1::DuplicateDependency { index: duplicate })
            );
        }
        let signals = [0x40, 0x80, 0xc0, 0x100, 0x140].map(address);
        for (index, completion) in signals.iter().copied().enumerate() {
            assert_eq!(
                AqlDependencyBarrierAndPacketV1::new_unpublished(&signals, completion),
                Err(AqlDependencyBarrierAndPacketErrorV1::CompletionAliasesDependency { index })
            );
        }
    }

    #[derive(Default)]
    struct Target {
        events: Vec<&'static str>,
        bytes: Option<[u8; 64]>,
        header: Option<u16>,
        fail_body: bool,
        fail_header: bool,
    }

    impl AqlDependencyBarrierAndPublicationTargetV1 for Target {
        type Error = &'static str;

        fn write_unpublished_dependency_barrier(
            &mut self,
            packet: &AqlDependencyBarrierAndPacketV1,
        ) -> Result<(), Self::Error> {
            self.events.push("body");
            assert!(packet.is_unpublished());
            if self.fail_body {
                return Err("body");
            }
            self.bytes = Some(packet.encode_unpublished_le());
            Ok(())
        }

        fn publish_dependency_barrier_release_header(
            &mut self,
            header: u16,
        ) -> Result<(), Self::Error> {
            self.events.push("header");
            assert!(self.bytes.is_some());
            if self.fail_header {
                return Err("header");
            }
            self.header = Some(header);
            Ok(())
        }
    }

    #[test]
    fn publication_pairs_body_with_exact_system_wait_for_prior_header() {
        let mut target = Target::default();
        prepared(&[0x1000, 0x2000])
            .publish_with(&mut target)
            .unwrap();
        assert_eq!(target.events, ["body", "header"]);
        assert_eq!(target.header, Some(3 | (1 << 8) | (2 << 9) | (2 << 11)));
        assert_eq!(target.header, Some(0x1503));
        assert_eq!(&target.bytes.unwrap()[..4], &[1, 0, 0, 0]);
    }

    #[test]
    fn publication_errors_stop_without_retry_or_later_header() {
        for fail_body in [true, false] {
            let mut target = Target {
                fail_body,
                fail_header: !fail_body,
                ..Target::default()
            };
            let error = prepared(&[0x40]).publish_with(&mut target).unwrap_err();
            assert_eq!(error, if fail_body { "body" } else { "header" });
            assert_eq!(
                target.events,
                if fail_body {
                    alloc::vec!["body"]
                } else {
                    alloc::vec!["body", "header"]
                }
            );
            assert_eq!(target.header, None);
        }
    }

    #[test]
    fn legacy_admission_stays_closed_to_the_new_header() {
        assert!(!crate::is_reviewed_aql_publication_v1(0x1503, 0));
        assert!(crate::is_reviewed_aql_publication_v1(0x1403, 0));
        assert!(!crate::is_reviewed_aql_publication_v1(0x1403, 1));
        let legacy = crate::AqlBarrierAndPacketV1::new_unpublished(address(0x9000)).unwrap();
        assert_eq!(&legacy.packet.encode_unpublished_le()[8..48], &[0; 40]);
        assert_eq!(
            crate::AQL_BARRIER_AND_ABI_SCHEMA_MANIFEST_SHA256_V1,
            "bdca900cd5c6eaccbddfc5a854e956382a08ce87bec4ccd5284baacf932cdfb5"
        );
    }

    #[test]
    fn manifest_hash_and_missing_authority_are_explicit() {
        assert_eq!(
            hex(&Sha256::digest(
                AQL_DEPENDENCY_BARRIER_AND_ABI_SCHEMA_MANIFEST_V1
            )),
            AQL_DEPENDENCY_BARRIER_AND_ABI_SCHEMA_MANIFEST_SHA256_V1
        );
        for boundary in [
            "no-native-admission",
            "mapped-all-participants",
            "retained-owner-and-mapping-lifetimes",
            "topology-currentness",
            "signal-kind-value-generation",
            "acyclic-progress",
        ] {
            assert!(AQL_DEPENDENCY_BARRIER_AND_ABI_SCHEMA_MANIFEST_V1.contains(boundary));
        }
    }
}

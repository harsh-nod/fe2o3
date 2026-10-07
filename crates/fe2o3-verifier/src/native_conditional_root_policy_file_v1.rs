//! Independent fixed-policy framing reuses the canonical source-bound roster.
use fe2o3_compiler_lineage::{
    MAX_NATIVE_CONDITIONAL_STORAGE_V1, NATIVE_CONDITIONAL_POLICY_ROSTER_WORKING_STORAGE_V1,
    NativeConditionalPolicyRosterRefV1 as Roster, read_native_conditional_policy_roster_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
};
use sha2::{Digest, Sha256};
use std::io;

pub const MAX_NATIVE_CONDITIONAL_ROOT_POLICY_FILE_BYTES_V1: usize = 131_688;
const MAX_POLICY_BYTES: usize = MAX_NATIVE_CONDITIONAL_ROOT_POLICY_FILE_BYTES_V1;
const MIN_BYTES: usize = 648;
const HEADER: usize = 72;
const MAGIC: &[u8; 8] = b"F3NCRP1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-CONDITIONAL-ROOT-POLICY-FILE/V1\0";
const SCRATCH: usize = NATIVE_CONDITIONAL_POLICY_ROSTER_WORKING_STORAGE_V1 + 4096;

struct View<'a> {
    roster: Roster<'a>,
}

/// Inert format validation only. This neither accepts the roster nor authenticates
/// the installed file. The fixed native deployment must independently pin its
/// raw SHA-256 and exact byte length and retain the original protected file.
pub fn validate_native_conditional_root_policy_file_v1(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> io::Result<()> {
    read(bytes, budget).map(|_| ())
}

/// Wraps an already encoded canonical roster for explicit administrative
/// provisioning. Only singleton gfx942:xnack-/wave64 conditional fill, production
/// history limits V1 and final-KIR-to-machine boundary 6 are selected. Acceptance
/// never follows from constructing these bytes or from a source-supplied roster.
/// Returns the full unreserved Vec/header charge; input backing remains prepaid.
pub fn encode_native_conditional_root_policy_file_v1(
    roster: &[u8],
    budget: &mut Budget<'_>,
) -> io::Result<(Vec<u8>, usize)> {
    let length = roster
        .len()
        .checked_add(HEADER + 32)
        .ok_or_else(|| io::Error::other("native policy extent overflow"))?;
    budget.charge_work(8).map_err(other)?;
    require(
        (MIN_BYTES..=MAX_POLICY_BYTES).contains(&length),
        "native policy extent",
    )?;
    let work = length
        .checked_mul(4)
        .ok_or_else(|| io::Error::other("native policy work overflow"))?;
    refundable_scope(budget, roster.len(), work, SCRATCH + length, |b| {
        let decoded = read_roster(roster, b)?;
        require(
            decoded.root_count() == 1,
            "native fill policy requires one root",
        )?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(other)?;
        require(
            bytes.capacity() == length,
            "native policy allocation capacity",
        )?;
        bytes.resize(length, 0);
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&(length as u32).to_le_bytes());
        bytes[24..26].copy_from_slice(&1u16.to_le_bytes()); // gfx942:xnack-, wave64
        bytes[26..28].copy_from_slice(&1u16.to_le_bytes()); // production history V1
        bytes[28..30].copy_from_slice(&6u16.to_le_bytes());
        bytes[32..40].copy_from_slice(&(roster.len() as u64).to_le_bytes());
        bytes[40..72].copy_from_slice(&Sha256::digest(roster));
        bytes[HEADER..length - 32].copy_from_slice(roster);
        let hash = checksum(&bytes[..length - 32]);
        bytes[length - 32..].copy_from_slice(&hash);
        Ok((bytes, length + size_of::<Vec<u8>>() + size_of::<usize>()))
    })
}

fn read<'a>(bytes: &'a [u8], budget: &mut Budget<'_>) -> io::Result<View<'a>> {
    budget.charge_work(8).map_err(other)?;
    require(
        (MIN_BYTES..=MAX_POLICY_BYTES).contains(&bytes.len()),
        "native policy extent",
    )?;
    let work = bytes
        .len()
        .checked_mul(4)
        .ok_or_else(|| io::Error::other("native policy work overflow"))?;
    refundable_scope(budget, bytes.len(), work, SCRATCH, |b| {
        require(
            &bytes[..8] == MAGIC
                && bytes[8..10] == 1u16.to_le_bytes()
                && bytes[10..12] == [0; 2]
                && bytes[12..16] == (bytes.len() as u32).to_le_bytes()
                && bytes[16..24] == [0; 8]
                && bytes[24..32] == [1, 0, 1, 0, 6, 0, 0, 0]
                && bytes[32..40] == ((bytes.len() - HEADER - 32) as u64).to_le_bytes()
                && bytes[bytes.len() - 32..] == checksum(&bytes[..bytes.len() - 32]),
            "noncanonical native policy profile",
        )?;
        let roster = &bytes[HEADER..bytes.len() - 32];
        require(
            bytes[40..72] == Sha256::digest(roster)[..],
            "native policy roster hash",
        )?;
        let roster = read_roster(roster, b)?;
        require(
            roster.root_count() == 1,
            "native fill policy requires one root",
        )?;
        Ok(View { roster })
    })
}
fn read_roster<'a>(bytes: &'a [u8], budget: &mut Budget<'_>) -> io::Result<Roster<'a>> {
    read_native_conditional_policy_roster_v1(bytes, MAX_NATIVE_CONDITIONAL_STORAGE_V1, |work| {
        budget.charge_work(work)
    })
    .map_err(|_| io::Error::other("canonical native policy roster refused"))
}
fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

/// Reconstructs native content using the caller's independently accepted policy
/// file bytes. This does not establish installation, source acceptance, currentness
/// or protected execution custody: callers must retain those separate original
/// owners. In particular, passing a file copied from the handoff does not make its
/// policy independently accepted. The source and embedded roster must match this
/// exact file before genuine V5 recovery is invoked.
///
/// Policy bytes and handoff backing/metadata are already prepaid on `budget`.
/// Returns the native recovery's full unreserved storage receipt. Failure is
/// terminal and preserves partial reservations; no refund scope encloses recovery.
pub fn recover_native_conditional_handoff_under_policy_file_v1(
    policy_bytes: &[u8],
    handoff: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV5,
    budget: &mut Budget<'_>,
) -> io::Result<(
    crate::RecoveredCompilerConditionalNativeSemanticHandoffV5,
    crate::RecoveredCompilerConditionalNativeSemanticHandoffStorageV5,
)> {
    budget
        .reserve_storage(size_of::<View<'_>>())
        .map_err(other)?;
    let view = read(policy_bytes, budget)?;
    let source = handoff.capsule().source_packet_bytes();
    budget
        .charge_work(source.len() + view.roster.canonical_bytes().len())
        .map_err(other)?;
    require(
        source.len() as u64 == view.roster.source_packet_len()
            && &<[u8; 32]>::from(Sha256::digest(source)) == view.roster.source_packet_sha256()
            && handoff.capsule().policy_roster_bytes() == view.roster.canonical_bytes(),
        "native source does not match independently accepted policy",
    )?;
    let (roster, storage) =
        crate::reconstruct_inert_native_conditional_policy_roster_in_original_account_v1(
            view.roster.canonical_bytes(),
            source,
            budget,
        )
        .map_err(other)?;
    budget
        .reserve_storage(storage.retained_storage())
        .map_err(other)?;
    budget
        .release_storage(size_of::<View<'_>>())
        .map_err(other)?;
    let result = roster
        .with_root_policies(budget, |roots, b| {
            crate::recover_compiler_conditional_native_semantic_handoff_in_original_account_v5(
                handoff,
                roots,
                fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryLimitsV1::production_v1(),
                fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
                b,
            )
            .map_err(|_| io::Error::other("independent native V5 recovery refused"))
        })
        .map_err(other)??;
    drop(roster);
    budget
        .release_storage(storage.retained_storage())
        .map_err(other)?;
    Ok(result)
}

fn other(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(error)
}
fn require(condition: bool, reason: &'static str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(reason))
    }
}
enum ScopeError {
    Resource(ResourceError),
    Io(io::Error),
}
impl From<ResourceError> for ScopeError {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}
fn refundable_scope<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    work: usize,
    scratch: usize,
    run: impl FnOnce(&mut Budget<'_>) -> io::Result<T>,
) -> io::Result<T> {
    budget
        .with_prepaid_scope(floor, 0, work, scratch, |b| run(b).map_err(ScopeError::Io))
        .map_err(|error| match error {
            ScopeError::Resource(error) => other(error),
            ScopeError::Io(error) => error,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_compiler_lineage::{
        NativeConditionalPolicyRootInputV1 as Root, NativeConditionalPolicyRosterInputV1 as Input,
        encode_native_conditional_policy_roster_v1,
    };
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    pub(super) fn roster(count: usize) -> Vec<u8> {
        let signers = [[1; 32]];
        let roots: Vec<_> = (0..count)
            .map(|index| Root {
                semantic_root: index as u32,
                kernel_binding: [2; 32],
                effect_signers: &signers,
                effect_toolchain: [[3; 32]; 5],
                formula_verifying_key: [4; 32],
                formula_toolchain: [[5; 32]; 5],
                formula_boundary: 2,
            })
            .collect();
        encode_native_conditional_policy_roster_v1(
            Input {
                source_packet: b"independently selected source",
                roots: &roots,
            },
            MAX_NATIVE_CONDITIONAL_STORAGE_V1,
            |_| Ok::<_, std::convert::Infallible>(()),
        )
        .unwrap()
    }
    #[test]
    fn native_policy_file_reuses_exact_roster_and_closed_profile() {
        let roster = roster(1);
        let mut work = Work::new(100_000_000);
        let mut b = Budget::new(&mut work, 1_000_000);
        b.reserve_storage(roster.len()).unwrap();
        let (bytes, storage) =
            encode_native_conditional_root_policy_file_v1(&roster, &mut b).unwrap();
        b.reserve_storage(storage).unwrap();
        assert_eq!(bytes.len(), MIN_BYTES);
        let view = read(&bytes, &mut b).unwrap();
        assert_eq!(view.roster.canonical_bytes(), roster);
        assert_eq!(view.roster.root_count(), 1);
        assert_eq!(
            view.roster.source_packet_sha256(),
            &<[u8; 32]>::from(Sha256::digest(b"independently selected source"))
        );
        assert!(!view.roster.grants_authority());
        let floor = b.storage();
        for offset in 0..bytes.len() {
            let mut changed = bytes.clone();
            changed[offset] ^= 1;
            assert!(read(&changed, &mut b).is_err(), "offset {offset}");
            assert_eq!(b.storage(), floor);
        }
        for offset in [8, 10, 16, 24, 26, 28, 30, 32, 40] {
            let mut changed = bytes.clone();
            changed[offset] ^= 1;
            let end = changed.len() - 32;
            let digest = checksum(&changed[..end]);
            changed[end..].copy_from_slice(&digest);
            assert!(read(&changed, &mut b).is_err(), "resealed {offset}");
        }
        assert!(read(&bytes[..bytes.len() - 1], &mut b).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(read(&trailing, &mut b).is_err());
        let mut legacy = bytes;
        legacy[..8].copy_from_slice(b"F3APMS1\0");
        assert!(read(&legacy, &mut b).is_err());
    }
    #[test]
    fn native_policy_file_rejects_multiroot_and_original_budget_denials() {
        let roster = roster(2);
        let mut work = Work::new(100_000_000);
        let mut b = Budget::new(&mut work, 1_000_000);
        b.reserve_storage(roster.len()).unwrap();
        assert!(encode_native_conditional_root_policy_file_v1(&roster, &mut b).is_err());
        let roster = self::roster(1);
        let (bytes, _) = encode_native_conditional_root_policy_file_v1(&roster, &mut b).unwrap();
        let floor = bytes.len();
        for work_limit in [0, 7, 8] {
            let mut work = Work::new(work_limit);
            let mut b = Budget::new(&mut work, 1_000_000);
            b.reserve_storage(floor).unwrap();
            assert!(read(&bytes, &mut b).is_err());
            assert!(b.failed_work().is_some());
            assert_eq!(b.storage(), floor);
        }
        let mut work = Work::new(100_000_000);
        let mut b = Budget::new(&mut work, floor + SCRATCH - 1);
        b.reserve_storage(floor).unwrap();
        assert!(read(&bytes, &mut b).is_err());
        assert!(b.failed_storage().is_some());
        assert_eq!(b.storage(), floor);
    }
}

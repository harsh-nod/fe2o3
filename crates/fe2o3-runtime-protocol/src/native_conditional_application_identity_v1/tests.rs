//! Signed wire fixtures and inert framing only, not a recovered V5 publication.
use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionAttestationStorageV3 as Storage,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionReceiptPublicationAckV3 as Ack,
    CompilerExecutionReceiptPublicationV3 as Publication,
    CompilerExecutionServiceLaunchManifestV3 as Launch,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

#[allow(dead_code)]
#[path = "../../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../fe2o3-compiler-execution-protocol/tests/support/native_receipt_fixture.rs"]
mod receipt_fixture;

fn retain<T, E: fmt::Debug>(result: std::result::Result<(T, Storage), E>, b: &mut Budget<'_>) -> T {
    let (value, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    value
}

struct Fixture {
    account: Owned,
    carriage: Carriage,
    handoff: Handoff,
    readiness: Vec<u8>,
}

impl Fixture {
    fn new(parent: u32, publication_nonce: u8, other_policy: bool) -> Self {
        let mut account = Owned::new(Work::new(100_000_000), 8_000_000);
        let (carriage, handoff, readiness) = account.with_budget(|b| {
            let policy_wire = fixture::policy_wire(3);
            let request_wire = fixture::request_wire(3);
            let receipt_wire = receipt_fixture::receipt_wire(3);
            b.reserve_storage(policy_wire.len() + request_wire.len() + receipt_wire.len())
                .unwrap();
            let policy = retain(Policy::decode(&policy_wire, b), b);
            let request = retain(Request::decode(&request_wire, b), b);
            let receipt = retain(Receipt::decode(&receipt_wire, b), b);
            let publication = retain(
                Publication::new([publication_nonce; 32], [0x82; 32], receipt, b),
                b,
            );
            let ack = retain(Ack::new(&publication, [0x83; 32], b), b);
            let carriage = retain(Carriage::new(policy, request, publication, ack, b), b);
            let mut launch_policy_wire = policy_wire;
            if other_policy {
                launch_policy_wire[32] ^= 1;
                fixture::seal(
                    &mut launch_policy_wire,
                    "COMPILER-EXECUTION-ISSUER-POLICY",
                    3,
                );
            }
            b.reserve_storage(launch_policy_wire.len()).unwrap();
            let launch_policy = retain(Policy::decode(&launch_policy_wire, b), b);
            let child = Client::new(1201, 1001, 1001).unwrap();
            let service = Service::new(2001, 2001).unwrap();
            let launch = retain(Launch::new(child, service, &launch_policy, b), b);
            let handoff = retain(
                Handoff::new(Client::new(parent, 1001, 1001).unwrap(), launch, b),
                b,
            );
            // The nested source fields are intentionally not semantic evidence.
            let readiness = framed_readiness(b"record", &carriage);
            b.reserve_storage(readiness.len()).unwrap();
            (carriage, handoff, readiness)
        });
        Self {
            account,
            carriage,
            handoff,
            readiness,
        }
    }

    fn bind(&mut self) -> Result<NativeConditionalApplicationBindingV1> {
        self.account.with_budget(|b| {
            NativeConditionalApplicationBindingV1::bind(
                &self.readiness,
                &self.handoff,
                &self.carriage,
                b,
            )
        })
    }
}

fn framed_readiness(record: &[u8], carriage: &Carriage) -> Vec<u8> {
    crate::conditional_worker_readiness_codec::encode(
        [
            record,
            b"claim",
            b"outer",
            b"transcript",
            carriage.canonical_bytes(),
        ],
        std::iter::empty::<&[u8]>(),
    )
    .unwrap()
}

fn decode(bytes: &[u8]) -> Result<NativeConditionalApplicationBindingV1> {
    let q = NativeConditionalApplicationBindingV1::decoding_quote();
    let mut work = Work::new(q.work());
    let mut b = Budget::new(&mut work, q.input_floor() + q.scratch());
    b.reserve_storage(q.input_floor()).unwrap();
    NativeConditionalApplicationBindingV1::decode(bytes, &mut b)
}

#[test]
fn canonical_binding_is_inert_and_round_trips_on_original_account() {
    let mut f = Fixture::new(1200, 0x81, false);
    let q = NativeConditionalApplicationBindingV1::binding_quote(
        f.readiness.len(),
        &f.handoff,
        &f.carriage,
    )
    .unwrap();
    f.account.with_budget(|b| {
        let storage = b.storage();
        let work = b.work();
        let ledger = b.work_ledger_identity_v1();
        let account = b.storage_account_identity_v1();
        let binding =
            NativeConditionalApplicationBindingV1::bind(&f.readiness, &f.handoff, &f.carriage, b)
                .unwrap();
        assert_eq!(b.storage(), storage);
        assert_eq!(b.work(), work + q.work());
        assert!(ledger == b.work_ledger_identity_v1());
        assert_eq!(account, b.storage_account_identity_v1());
        assert_eq!(binding.readiness_byte_len(), f.readiness.len() as u64);
        assert_eq!(
            binding.readiness_sha256(),
            <[u8; 32]>::from(Sha256::digest(&f.readiness))
        );
        assert_eq!(
            binding.compiler_handoff_identity(),
            *f.handoff.identity().as_bytes()
        );
        assert!(!binding.grants_load_authority());
        assert!(!binding.grants_launch_authority());
        assert_eq!(decode(binding.canonical_bytes()).unwrap(), binding);
        assert_ne!(binding.identity().as_bytes(), &[0; 32]);
        b.reserve_storage(q.retained_storage()).unwrap();
        assert_eq!(
            b.storage(),
            storage + size_of::<NativeConditionalApplicationBindingV1>()
        );
    });
}

#[test]
fn every_wire_byte_and_length_is_bound() {
    let binding = Fixture::new(1200, 0x81, false).bind().unwrap();
    for offset in 0..BYTES {
        let mut wire = *binding.canonical_bytes();
        wire[offset] ^= 1;
        assert!(decode(&wire).is_err(), "byte {offset}");
    }
    for length in 0..BYTES {
        assert!(decode(&binding.canonical_bytes()[..length]).is_err());
    }
    let mut longer = binding.canonical_bytes().to_vec();
    longer.push(0);
    assert!(decode(&longer).is_err());
}

#[test]
fn resealed_invalid_headers_lengths_and_identities_are_rejected() {
    let binding = Fixture::new(1200, 0x81, false).bind().unwrap();
    for axis in 0..9 {
        let mut wire = *binding.canonical_bytes();
        match axis {
            0 => wire[8..10].copy_from_slice(&2u16.to_le_bytes()),
            1 => wire[10] = 1,
            2 => wire[16..HEADER].fill(0),
            3 => wire[16..HEADER].copy_from_slice(
                &(MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 as u64 + 1).to_le_bytes(),
            ),
            n => wire[HEADER + (n - 4) * 32..HEADER + (n - 3) * 32].fill(0),
        }
        let identity = checksum(&wire[..CHECKSUM]);
        wire[CHECKSUM..].copy_from_slice(&identity);
        assert!(decode(&wire).is_err(), "axis {axis}");
    }
}

#[test]
fn parent_readiness_and_publication_are_distinct_identity_axes() {
    let baseline = Fixture::new(1200, 0x81, false).bind().unwrap().identity();
    assert_ne!(
        Fixture::new(1202, 0x81, false).bind().unwrap().identity(),
        baseline
    );
    assert_ne!(
        Fixture::new(1200, 0x84, false).bind().unwrap().identity(),
        baseline
    );
    let mut changed = Fixture::new(1200, 0x81, false);
    changed.readiness = framed_readiness(b"Record", &changed.carriage);
    assert_ne!(changed.bind().unwrap().identity(), baseline);
}

#[test]
fn policy_and_embedded_carriage_substitution_fail_closed() {
    assert!(matches!(
        Fixture::new(1200, 0x81, true).bind(),
        Err(Error::PolicyMismatch)
    ));
    let mut f = Fixture::new(1200, 0x81, false);
    let other = Fixture::new(1200, 0x84, false);
    f.readiness = framed_readiness(b"record", &other.carriage);
    assert!(matches!(f.bind(), Err(Error::CarriageMismatch)));
    f.readiness[0] ^= 1;
    assert!(matches!(f.bind(), Err(Error::Readiness(_))));
}

#[test]
fn exact_quotes_preserve_accounts_and_record_resource_denials() {
    let f = Fixture::new(1200, 0x81, false);
    let q = NativeConditionalApplicationBindingV1::binding_quote(
        f.readiness.len(),
        &f.handoff,
        &f.carriage,
    )
    .unwrap();
    // Separately prepaid synthetic codec inputs exercise each numerical bound.
    for case in 0..4 {
        let floor = q.input_floor() - usize::from(case == 3);
        let mut work = Work::new(q.work() - usize::from(case == 1));
        let mut b = Budget::new(&mut work, floor + q.scratch() - usize::from(case == 2));
        b.reserve_storage(floor).unwrap();
        let account = b.storage_account_identity_v1();
        let ledger = b.work_ledger_identity_v1();
        let result = NativeConditionalApplicationBindingV1::bind(
            &f.readiness,
            &f.handoff,
            &f.carriage,
            &mut b,
        );
        assert_eq!(result.is_ok(), case == 0, "case {case}");
        assert_eq!(b.storage(), floor);
        assert_eq!(b.storage_account_identity_v1(), account);
        assert!(b.work_ledger_identity_v1() == ledger);
        if case == 0 {
            assert_eq!(b.work(), q.work());
        }
        if case == 1 {
            assert!(b.failed_work().is_some());
        }
        if case == 2 {
            assert!(b.failed_storage().is_some());
        }
    }
}

#[test]
fn decoding_exact_quote_and_one_short_are_enforced() {
    let binding = Fixture::new(1200, 0x81, false).bind().unwrap();
    let q = NativeConditionalApplicationBindingV1::decoding_quote();
    for case in 0..4 {
        let floor = q.input_floor() - usize::from(case == 3);
        let mut work = Work::new(q.work() - usize::from(case == 1));
        let mut b = Budget::new(&mut work, floor + q.scratch() - usize::from(case == 2));
        b.reserve_storage(floor).unwrap();
        let result =
            NativeConditionalApplicationBindingV1::decode(binding.canonical_bytes(), &mut b);
        assert_eq!(result.is_ok(), case == 0);
        assert_eq!(b.storage(), floor);
        if case == 1 {
            assert!(b.failed_work().is_some());
        }
        if case == 2 {
            assert!(b.failed_storage().is_some());
        }
    }
}

#[test]
fn quote_length_edges_and_early_refusal_stay_bounded() {
    let mut f = Fixture::new(1200, 0x81, false);
    for (length, accepted) in [
        (0, false),
        (MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5, true),
        (MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 + 1, false),
        (usize::MAX, false),
    ] {
        assert_eq!(
            NativeConditionalApplicationBindingV1::binding_quote(length, &f.handoff, &f.carriage,)
                .is_ok(),
            accepted
        );
    }
    f.account.with_budget(|b| {
        let storage = b.storage();
        let work = b.work();
        assert!(matches!(
            NativeConditionalApplicationBindingV1::bind(&[], &f.handoff, &f.carriage, b,),
            Err(Error::Length)
        ));
        assert_eq!(b.storage(), storage);
        assert_eq!(b.work(), work + ENTRY);
    });
}

#[test]
fn association_refusal_retains_work_peak_and_prior_denial_history() {
    let f = Fixture::new(1200, 0x81, true);
    let q = NativeConditionalApplicationBindingV1::binding_quote(
        f.readiness.len(),
        &f.handoff,
        &f.carriage,
    )
    .unwrap();
    let mut work = Work::new(q.work());
    let limit = q.input_floor() + q.scratch();
    let mut b = Budget::new(&mut work, limit);
    b.reserve_storage(q.input_floor()).unwrap();
    assert!(b.reserve_storage(limit).is_err());
    let denial = b.failed_storage();
    let identity = b.storage_account_identity_v1();
    let result =
        NativeConditionalApplicationBindingV1::bind(&f.readiness, &f.handoff, &f.carriage, &mut b);
    assert!(matches!(result, Err(Error::PolicyMismatch)));
    assert_eq!(b.work(), q.work());
    assert_eq!(b.peak_storage(), limit);
    assert_eq!(b.storage(), q.input_floor());
    assert_eq!(b.failed_storage(), denial);
    assert_eq!(b.storage_account_identity_v1(), identity);
}

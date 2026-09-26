//! Inert component transcripts, not protected occurrence or durable receipt fixtures.
use fe2o3_compiler_execution_protocol::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::mem::size_of;

#[path = "support/native_attestation_fixture.rs"]
mod fixture;
#[path = "support/native_publication_fixture.rs"]
mod publication_fixture;
#[path = "support/native_receipt_fixture.rs"]
mod receipt_fixture;
use fixture::*;
use publication_fixture::*;
use receipt_fixture::*;

type Policy = CompilerExecutionIssuerPolicyV3;
type Request = CompilerExecutionAttestationRequestV3;
type Receipt = CompilerExecutionAttestationReceiptV3;
type Publication = CompilerExecutionReceiptPublicationV3;
type Ack = CompilerExecutionReceiptPublicationAckV3;
type Carriage = CompilerExecutionReceiptCarriageV3;
type Error = CompilerExecutionReceiptPublicationErrorV3;
type Framing = CompilerExecutionReceiptPublicationErrorV1;
type Attestation = CompilerExecutionAttestationErrorV3;
type AttestationFraming = CompilerExecutionAttestationErrorV1;
const LIMIT: usize = 1_000_000;
const WORK: usize = 10_000_000;
const UW: usize = COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V3;
const UDW: usize = COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V3;
const US: usize = COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V3;
const UDS: usize = COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V3;
const AW: usize = COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V3;
const AS: usize = COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V3;
const CW: usize = COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V3;
const CCW: usize = COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_WORK_V3;
const CDW: usize = COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V3;
const CS: usize = COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V3;
const CCS: usize = COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_STORAGE_V3;
const CDS: usize = COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V3;

// Fixture preparation for isolated component probes. Not a production-account transfer.
fn run<T>(floor: usize, f: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>) -> Result<T, Error> {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let result = f(&mut b);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    result
}
fn publication() -> Publication {
    run(584, |b| Publication::decode(&publication_wire(3), b))
        .unwrap()
        .0
}
fn ack() -> Ack {
    run(288, |b| Ack::decode(&ack_wire(3), b)).unwrap().0
}
fn carriage(bytes: &[u8]) -> Result<Carriage, Error> {
    run(bytes.len(), |b| Carriage::decode(bytes, b)).map(|(c, _)| c)
}
fn policy() -> Policy {
    run(216, |b| Ok(Policy::decode(&policy_wire(3), b)?))
        .unwrap()
        .0
}
fn request() -> Request {
    run(946, |b| Ok(Request::decode(&request_wire(3), b)?))
        .unwrap()
        .0
}
fn receipt() -> Receipt {
    run(400, |b| Ok(Receipt::decode(&receipt_wire(3), b)?))
        .unwrap()
        .0
}
fn decode(kind: usize, bytes: &[u8]) -> Result<(), Error> {
    run(bytes.len(), |b| match kind {
        0 => Publication::decode(bytes, b).map(|_| ()),
        1 => Ack::decode(bytes, b).map(|_| ()),
        _ => Carriage::decode(bytes, b).map(|_| ()),
    })
}

#[test]
fn conditional_carriage_v3_independent_wire_and_single_account_transfer() {
    let (pwire, qwire, rwire) = (policy_wire(3), request_wire(3), receipt_wire(3));
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let inputs = pwire.len() + qwire.len() + rwire.len();
    b.reserve_storage(inputs).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (p, s) = Policy::decode(&pwire, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (q, s) = Request::decode(&qwire, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (r, s) = Receipt::decode(&rwire, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let floor = b.storage();
    let inherited = r.retained_storage();
    let (u, s) = Publication::new(JOURNAL, OCCURRENCE, r, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(u.retained_storage(), inherited + s.additional_storage());
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(u.canonical_bytes(), &publication_wire(3));
    assert!(!u.proves_durable_publication());
    assert!(!u.grants_compiler_authority());
    let (a, s) = Ack::new(&u, WORKER, &mut b).unwrap();
    assert_eq!(s.additional_storage(), a.retained_storage());
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(a.canonical_bytes(), &ack_wire(3));
    assert!(!a.proves_durable_publication());
    assert!(!a.grants_compiler_authority());
    let inherited =
        p.retained_storage() + q.retained_storage() + u.retained_storage() + a.retained_storage();
    let floor = b.storage();
    let (c, s) = Carriage::new(p, q, u, a, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(c.retained_storage(), inherited + s.additional_storage());
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(c.canonical_bytes(), &carriage_wire(3));
    assert!(c.requires_protected_policy_verification());
    assert!(!c.grants_compiler_authority());
    assert!(!c.grants_load_authority());
    assert!(!c.grants_launch_authority());
    assert_eq!(c, carriage(&carriage_wire(3)).unwrap());
    assert_eq!(
        b.work(),
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3
            + COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V3
            + COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V3
            + UW
            + AW
            + CCW
    );
    assert!(b.work_ledger_identity_v1() == ledger);
    let paid = c.retained_storage();
    drop(c);
    b.release_storage(paid).unwrap();
    assert_eq!(b.storage(), inputs);
}

#[test]
fn conditional_carriage_v3_v2_layout_and_quote_parity() {
    for (v2, v3, exact) in [
        (
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V2,
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V3,
            584,
        ),
        (
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V2,
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V3,
            288,
        ),
        (
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V2,
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3,
            2090,
        ),
        (COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V2, UW, 18_696),
        (
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2,
            UDW,
            170_768,
        ),
        (
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2,
            AW,
            9_224,
        ),
        (COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2, CW, 66_888),
        (
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_WORK_V2,
            CCW,
            224_088,
        ),
        (
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V2,
            CDW,
            475_656,
        ),
    ] {
        assert_eq!(v2, exact);
        assert_eq!(v3, exact);
    }
    for (v2, v3) in [
        (COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V2, US),
        (
            COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2,
            UDS,
        ),
        (COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2, AS),
        (COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2, CS),
        (
            COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_STORAGE_V2,
            CCS,
        ),
        (COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V2, CDS),
        (
            size_of::<CompilerExecutionReceiptPublicationV2>(),
            size_of::<Publication>(),
        ),
        (
            size_of::<CompilerExecutionReceiptPublicationAckV2>(),
            size_of::<Ack>(),
        ),
        (
            size_of::<CompilerExecutionReceiptCarriageV2>(),
            size_of::<Carriage>(),
        ),
        (
            size_of::<CompilerExecutionReceiptPublicationErrorV2>(),
            size_of::<Error>(),
        ),
    ] {
        assert_eq!(v2, v3);
    }
}

#[path = "support/conditional_publication_v3_mutations.rs"]
mod mutations;
#[path = "support/conditional_publication_v3_resources.rs"]
mod resources;

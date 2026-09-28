//! Independent fixed-layout construction; no production codec calls or authority.
use super::fixture::{header, policy_wire, request_wire, seal};
use super::receipt_fixture::receipt_wire;

pub const JOURNAL: [u8; 32] = [0x81; 32];
pub const OCCURRENCE: [u8; 32] = [0x82; 32];
pub const WORKER: [u8; 32] = [0x83; 32];

pub fn publication_wire(version: u16) -> [u8; 584] {
    let receipt = receipt_wire(version);
    let mut bytes = [0; 584];
    header(
        &mut bytes,
        match version {
            1 => b"F2O3CES1",
            2 => b"F2O3CES2",
            3 => b"F2O3CES3",
            _ => panic!("unsupported fixture family"),
        },
        version,
    );
    bytes[24..56].copy_from_slice(&receipt[64..96]);
    bytes[56..88].copy_from_slice(&JOURNAL);
    bytes[88..120].copy_from_slice(&OCCURRENCE);
    bytes[120..152].copy_from_slice(&receipt[368..]);
    bytes[152..552].copy_from_slice(&receipt);
    seal(
        &mut bytes,
        "COMPILER-EXECUTION-RECEIPT-PUBLICATION",
        version,
    );
    bytes
}
pub fn ack_wire(version: u16) -> [u8; 288] {
    let publication = publication_wire(version);
    let receipt = receipt_wire(version);
    let mut bytes = [0; 288];
    header(
        &mut bytes,
        match version {
            1 => b"F2O3CEA1",
            2 => b"F2O3CEA2",
            3 => b"F2O3CEA3",
            _ => panic!("unsupported fixture family"),
        },
        version,
    );
    bytes[24..152].copy_from_slice(&publication[24..152]);
    bytes[152..184].copy_from_slice(&publication[552..]);
    bytes[184..216].copy_from_slice(&WORKER);
    bytes[216..224].copy_from_slice(&receipt[200..208]);
    bytes[224..256].copy_from_slice(&receipt[240..272]);
    seal(
        &mut bytes,
        "COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK",
        version,
    );
    bytes
}
pub fn carriage_wire(version: u16) -> [u8; 2090] {
    let mut bytes = [0; 2090];
    header(
        &mut bytes,
        match version {
            1 => b"F2O3CRG1",
            2 => b"F2O3CRG2",
            3 => b"F2O3CRG3",
            _ => panic!("unsupported fixture family"),
        },
        version,
    );
    bytes[24..240].copy_from_slice(&policy_wire(version));
    bytes[240..1186].copy_from_slice(&request_wire(version));
    bytes[1186..1770].copy_from_slice(&publication_wire(version));
    bytes[1770..2058].copy_from_slice(&ack_wire(version));
    seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT-CARRIAGE", version);
    bytes
}

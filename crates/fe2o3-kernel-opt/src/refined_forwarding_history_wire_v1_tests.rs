use super::*;
use crate::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Function, InertCanonicalKirTransitionGraphIdentityV1 as Identity, MemoryAccess, Module,
    Operation, OperationKind as Kind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};

#[path = "refined_forwarding_history_decode_v1_tests.rs"]
mod decoded;
#[path = "refined_forwarding_history_nonzero_v1_tests.rs"]
mod nonzero;
const WORK: usize = 1_000_000_000;
const STORAGE: usize = MAX_REFINED_FORWARDING_HISTORY_STORAGE_V1;

fn module(nonempty: bool) -> Module {
    let mut m = Module::new("inert-twelve-role-history");
    if !nonempty {
        return m;
    }
    let mut block = BasicBlock::new(BlockId(0));
    for (id, lhs, rhs) in [(3, 0, 1), (4, 1, 0)] {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        ));
        block.operations.push(Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(2),
                value: ValueId(id),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    m.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
            ],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    m
}

// Genuine component owners and public producers, not source/rustc authority.
// The P7 transcript is inert fixture encoding of their actual typed rows.
fn with_history<T>(nonempty: bool, run: impl FnOnce(Inputs<'_>, usize) -> T) -> T {
    with_history_module(&module(nonempty), usize::from(nonempty), run)
}
use crate::test_support::with_refined_forwarding_history_module_v1 as with_history_module;
fn encode(inputs: Inputs<'_>, floor: usize) -> InertRefinedForwardingHistoryBytesV1 {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(floor).unwrap();
    let result = encode_refined_forwarding_history_v1(inputs, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    result
}
fn raw(fields: &[&[u8]; 27]) -> Vec<u8> {
    let len = 248 + fields.iter().map(|s| s.len()).sum::<usize>();
    let mut out = vec![0; 248];
    out[..16].copy_from_slice(b"F2RFH1\0\0\x01\0\x01\0\xf8\0\0\0");
    out[16..24].copy_from_slice(&(len as u64).to_le_bytes());
    out[24] = 27;
    for (i, f) in fields.iter().enumerate() {
        out[32 + i * 8..40 + i * 8].copy_from_slice(&(f.len() as u64).to_le_bytes());
        out.extend_from_slice(f);
    }
    out
}
fn read(bytes: &[u8]) -> Result<InertRefinedForwardingHistoryRefV1<'_>, Error> {
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    b.reserve_storage(bytes.len()).unwrap();
    read_refined_forwarding_history_v1(bytes, &mut b)
}
fn site(n: u32) -> Site {
    Site {
        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(7),
            block: 11,
        },
        operation: n,
    }
}

#[test]
fn history_wire_independent_literal_header_is_framing_not_graph_authority() {
    const HEADER_HEX: &str = "463252464831000001000100f80000003a070000000000001b000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000c80000000000000004000000000000000001000000000000a00100000000000001000000000000008001000000000000a8000000000000000400000000000000040000000000000004000000000000000400000000000000040000000000000004000000000000008800000000000000";
    let mut bytes = HEADER_HEX
        .as_bytes()
        .chunks_exact(2)
        .map(|h| u8::from_str_radix(std::str::from_utf8(h).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(bytes.len(), 248);
    bytes.extend([7; 12]);
    bytes.push(0);
    bytes.extend([0; 200]);
    bytes.extend([0; 4]);
    bytes.extend([0; 256]);
    bytes.extend([0; 416]);
    bytes.push(0);
    bytes.extend([0; 384]);
    bytes.extend(40u32.to_le_bytes());
    bytes.extend(b"gpu-commutative-bitwise-dominance-cse-v1");
    bytes.extend([0; 80 + 36 + 8]);
    bytes.extend([0; 6 * 4]);
    for n in 1u64..=17 {
        bytes.extend(n.to_le_bytes());
    }
    assert_eq!(bytes.len(), 1850);
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    b.reserve_storage(bytes.capacity()).unwrap();
    let frame = read_refined_forwarding_history_v1(&bytes, &mut b).unwrap();
    b.reserve_storage(frame.storage().retained_storage())
        .unwrap();
    assert_eq!(frame.row_count(), 0);
    assert_eq!(frame.fields.len(), 27);
    assert_eq!(frame.limits.refinement.functions, 1);
    assert_eq!(frame.limits.forwarding.control_flow.analysis_work, 17);
    assert_eq!(raw(&frame.fields), bytes);
    assert!(matches!(
        materialize_refined_forwarding_history_v1(&frame, &mut b),
        Err(Error::Admission {
            role: RefinedForwardingHistoryRoleV1::B,
            ..
        })
    ));
}

#[test]
fn history_wire_full_twelve_roles_equal_bytes_and_original_limits_roundtrip() {
    for nonempty in [false, true] {
        with_history(nonempty, |inputs, floor| {
            let wire = encode(inputs, floor);
            let frame = read(wire.canonical_bytes()).unwrap();
            assert_eq!(frame.limits(), inputs.limits);
            let owners = [
                inputs.prefix.prefix.prefix.prefix.input,
                inputs.prefix.prefix.prefix.prefix.intermediate,
                inputs.prefix.prefix.prefix.prefix.stored,
                inputs.prefix.prefix.prefix.prefix.output,
                inputs.prefix.prefix.prefix.output,
                inputs.prefix.prefix.output,
                inputs.prefix.output,
                inputs.promoted,
                inputs.preheaders,
                inputs.licm,
                inputs.refined,
                inputs.output,
            ];
            for (i, owner) in owners.into_iter().enumerate() {
                assert_eq!(frame.fields[i], owner.canonical().canonical_bytes());
            }
            assert_eq!(raw(&frame.fields), wire.canonical_bytes());
            assert_eq!(frame.fields[18][16..272], *frame.fields[15]);
            assert_eq!(frame.fields[19][4..44], *PASS.as_bytes());
            assert!(!frame.grants_authority());
            if !nonempty {
                assert!(frame.fields[..12].iter().all(|b| *b == frame.fields[0]));
            }
        });
    }
}

#[test]
fn history_wire_all_header_fields_truncations_and_slot_extents_are_closed() {
    with_history(false, |inputs, floor| {
        let wire = encode(inputs, floor);
        let bytes = wire.canonical_bytes();
        for at in 0..32 {
            let mut bad = bytes.to_vec();
            bad[at] ^= 128;
            assert!(read(&bad).is_err(), "header {at}");
        }
        for i in 0..27 {
            let mut bad = bytes.to_vec();
            bad[32 + 8 * i..40 + 8 * i].fill(255);
            assert!(read(&bad).is_err());
        }
        for len in 0..bytes.len() {
            assert!(read(&bytes[..len]).is_err(), "truncation {len}");
        }
        let mut extra = bytes.to_vec();
        extra.push(0);
        assert!(read(&extra).is_err());
        let frame = read(bytes).unwrap();
        for i in 0..27 {
            let mut fields = frame.fields;
            fields[i] = &[];
            assert!(read(&raw(&fields)).is_err(), "missing {i}");
        }
    });
}

#[test]
fn history_wire_literal_rows_preserve_every_tag_and_exact_site_without_sorting() {
    fn check<T: rows::Row + std::fmt::Debug + PartialEq>(rows: &[T], widths: &[usize]) {
        let mut bytes = vec![0; 4 + widths.iter().sum::<usize>()];
        rows::encode(rows, &mut bytes).unwrap();
        assert_eq!(&bytes[..4], &(rows.len() as u32).to_le_bytes());
        let mut c = rows::Reader {
            bytes: &bytes,
            pos: 4,
        };
        for row in rows {
            let at = c.pos;
            assert_eq!(&T::read(&mut c).unwrap(), row);
            assert_eq!(c.pos - at, row.width());
        }
        c.done().unwrap();
        for end in 0..bytes.len() {
            assert!(rows::scan::<T>(&bytes[..end]).is_err());
        }
    }
    check(
        &[Load {
            first: site(9),
            load: site(1),
        }],
        &[24],
    );
    check(&[site(9), site(1), site(1)], &[12, 12, 12]);
    check(
        &[
            Promotion {
                input: site(3),
                output: site(5),
                kind: fe2o3_kernel_analysis::CanonicalKirPrivateCellOriginKindV1::Retained,
            },
            Promotion {
                input: site(4),
                output: site(6),
                kind: fe2o3_kernel_analysis::CanonicalKirPrivateCellOriginKindV1::LoadCopy {
                    allocation: site(1),
                    previous_store: site(2),
                    stored_value: ValueId(99),
                },
            },
        ],
        &[25, 53],
    );
    check(
        &[Preheader {
            header: site(0).block,
            preheader: site(1).block,
        }],
        &[16],
    );
    check(
        &[
            Licm {
                input: site(4),
                output: site(1),
                hoist: None,
            },
            Licm {
                input: site(5),
                output: site(2),
                hoist: Some(fe2o3_kernel_analysis::CanonicalKirLicmHoistV1 {
                    header: site(0).block,
                    sequence: 3,
                }),
            },
        ],
        &[25, 37],
    );
    check(
        &[
            Refinement::Unchanged {
                input: site(1),
                output: site(1),
            },
            Refinement::CheckedAddSplit {
                input: site(2),
                sum_output: site(2),
                false_output: site(3),
                induction_row_ordinal: 17,
            },
        ],
        &[25, 45],
    );
    check(
        &[
            Forwarding {
                input: site(1),
                output: site(1),
                store: None,
            },
            Forwarding {
                input: site(2),
                output: site(2),
                store: Some(site(1)),
            },
        ],
        &[25, 37],
    );
    let mut literal = [0u8; 29];
    literal[..4].copy_from_slice(&1u32.to_le_bytes());
    for (offset, value) in [(4, 7u32), (8, 11), (12, 3), (16, 7), (20, 11), (24, 5)] {
        literal[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    assert_eq!(rows::scan::<Promotion>(&literal).unwrap(), 1);
    for bad in 2..=255 {
        literal[28] = bad;
        assert!(matches!(rows::scan::<Promotion>(&literal), Err(Error::Tag)));
    }
}

#[test]
fn history_wire_all_seventeen_limits_are_exact_without_defaults_or_clamping() {
    let mut bytes = [0; 136];
    for (i, part) in bytes.chunks_exact_mut(8).enumerate() {
        part.copy_from_slice(&(i as u64 + 2).to_le_bytes());
    }
    let parsed = rows::read_limits(&bytes).unwrap();
    let mut encoded = [0; 136];
    rows::write_limits(parsed, &mut encoded).unwrap();
    assert_eq!(encoded, bytes);
    for i in 0..17 {
        let mut changed = bytes;
        changed[i * 8..i * 8 + 8].copy_from_slice(&0u64.to_le_bytes());
        let limits = rows::read_limits(&changed).unwrap();
        assert_ne!(limits, parsed);
        let mut exact = [0; 136];
        rows::write_limits(limits, &mut exact).unwrap();
        assert_eq!(exact, changed);
    }
    assert!(rows::read_limits(&bytes[..135]).is_err());
}

#[test]
fn history_wire_each_optional_or_variant_tag_is_closed_and_never_deduplicated() {
    let mut optional = [0; 29];
    optional[..4].copy_from_slice(&1u32.to_le_bytes());
    for tag in 2..=255 {
        optional[28] = tag;
        assert!(matches!(
            rows::scan::<Promotion>(&optional),
            Err(Error::Tag)
        ));
        assert!(matches!(rows::scan::<Licm>(&optional), Err(Error::Tag)));
        assert!(matches!(
            rows::scan::<Forwarding>(&optional),
            Err(Error::Tag)
        ));
        let mut variant = optional;
        variant[4] = tag;
        assert!(matches!(
            rows::scan::<Refinement>(&variant),
            Err(Error::Tag)
        ));
    }
    for count in [0u32, 2, u32::MAX] {
        optional[..4].copy_from_slice(&count.to_le_bytes());
        optional[28] = 0;
        assert!(rows::scan::<Promotion>(&optional).is_err());
    }
    let repeated = [site(1), site(1)];
    let mut bytes = [0; 28];
    rows::encode(&repeated, &mut bytes).unwrap();
    assert_eq!(rows::scan::<Site>(&bytes).unwrap(), 2);
    let mut w = Work::new(WORK);
    let mut b = Budget::new(&mut w, STORAGE);
    let result: Result<Vec<Site>, Error> = resources::scoped(&mut b, |m| {
        m.reserve(std::mem::size_of::<Vec<Site>>())?;
        rows::decode(&bytes, m).map(|(rows, _)| rows)
    });
    assert_eq!(result.unwrap(), repeated);
}

#[test]
fn history_wire_aggregate_caps_and_foreign_p7_p8_fields_refuse_before_materialization() {
    with_history(false, |inputs, floor| {
        let wire = encode(inputs, floor);
        let frame = read(wire.canonical_bytes()).unwrap();
        let huge = vec![0; MAX_REFINED_FORWARDING_HISTORY_GRAPH_BYTES_V1 / 12 + 1];
        let mut fields = frame.fields;
        fields[..12].fill(&huge);
        assert!(matches!(read(&raw(&fields)), Err(Error::Limit)));
        let mut p7 = fields[18].to_vec();
        p7[16] ^= 1;
        fields = frame.fields;
        fields[18] = &p7;
        assert!(matches!(read(&raw(&fields)), Err(Error::Field(18))));
        let mut tail = frame.fields[19].to_vec();
        tail[4] ^= 1;
        fields = frame.fields;
        fields[19] = &tail;
        assert!(matches!(read(&raw(&fields)), Err(Error::Field(19))));
        let count = (MAX_REFINED_FORWARDING_HISTORY_ROWS_V1 as u32 + 1).to_le_bytes();
        fields = frame.fields;
        fields[20] = &count;
        assert!(matches!(read(&raw(&fields)), Err(Error::Rows)));
    });
}

#[test]
fn history_wire_encoder_refuses_conflicting_p7_rows_and_tail_identities() {
    with_history(true, |inputs, floor| {
        let mut bad = inputs;
        bad.prefix.continuation.input =
            Identity::from_verified(inputs.prefix.output.canonical().identity());
        let mut w = Work::new(WORK);
        let mut b = Budget::new(&mut w, STORAGE);
        b.reserve_storage(floor).unwrap();
        assert!(matches!(
            encode_refined_forwarding_history_v1(bad, &mut b),
            Err(Error::TailIdentity)
        ));
        assert!(
            !inputs
                .prefix
                .prefix
                .continuation
                .retained_operations
                .is_empty()
        );
        let mut bad = inputs;
        bad.prefix.prefix.continuation.retained_operations = &[];
        assert!(matches!(
            encode_refined_forwarding_history_v1(bad, &mut b),
            Err(Error::Policy7Rows)
        ));
        assert_eq!(b.storage(), floor);
    });
}

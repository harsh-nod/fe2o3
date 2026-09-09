use super::*;

type Evidence = InertCanonicalSemanticCallExpansionEvidenceV1;
type EvidenceError = SemanticCallExpansionEvidenceErrorV1;

fn expand(source: &AdmittedInertSemanticMirV1) -> SemanticCallExpansionV1 {
    SemanticCallExpansionV1::try_new(source, SemanticCallExpansionLimitsV1::default()).unwrap()
}

fn roundtrip(source: &AdmittedInertSemanticMirV1) -> Evidence {
    let expansion = expand(source);
    let evidence = Evidence::from_checked_expansion(source, &expansion).unwrap();
    let decoded = Evidence::decode(evidence.canonical_bytes()).unwrap();
    assert_eq!(decoded, evidence);
    assert_eq!(
        decoded.source_semantic_sha256(),
        source.semantic_sha256().as_bytes()
    );
    assert_eq!(decoded.expansion_identity(), expansion.identity());
    assert_ne!(decoded.identity(), expansion.identity());
    assert_eq!(decoded.work_units(), expansion.work_units());
    assert!(!decoded.grants_proof_or_artifact_authority());
    assert_eq!(decoded.roots().len(), expansion.roots().len());
    for root in expansion.roots() {
        let record = decoded.root(root.root()).unwrap();
        assert_eq!(record.source_body(), root.source_body());
        assert_eq!(
            record.source_root_identity(),
            source.functions()[root.root().index() as usize].identity()
        );
        assert_eq!(record.identity(), root.identity());
        assert_ne!(record.identity(), decoded.expansion_identity());
        assert_eq!(record.expanded_function_identity(), root.body().identity());
        assert_eq!(record.instances(), root.instances());
        assert_eq!(record.local_origins(), root.local_origins());
        assert_eq!(record.block_origins(), root.block_origins());
        assert_eq!(record.has_expanded_calls(), root.has_expanded_calls());
        for (index, local) in root.body().locals().iter().enumerate() {
            assert_eq!(
                record.local_identity(SemanticLocalIdV1::from_index(index as u32)),
                Some(local.identity().as_bytes())
            );
        }
        for (index, block) in root.body().blocks().iter().enumerate() {
            assert_eq!(
                record.block_identity(SemanticBlockIdV1::from_index(index as u32)),
                Some(block.identity().as_bytes())
            );
        }
        assert!(
            record
                .local_identity(SemanticLocalIdV1::from_index(u32::MAX))
                .is_none()
        );
        assert!(
            record
                .block_identity(SemanticBlockIdV1::from_index(u32::MAX))
                .is_none()
        );
    }
    assert!(decoded.root(function_id(u32::MAX)).is_none());
    decoded
        .verify_against_checked_expansion(source, &expansion)
        .unwrap();
    decoded
}

fn caller(index: u32, root: bool, callee: u32) -> SemanticFunctionDeclV1 {
    function(
        index,
        root,
        false,
        vec![
            block(
                0,
                vec![],
                call(callee, 1, false, SemanticUnwindActionV1::Unreachable),
            ),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
}

fn mixed() -> AdmittedInertSemanticMirV1 {
    admit(
        vec![
            caller(0, true, 1),
            leaf(1),
            function(
                2,
                true,
                false,
                vec![block(0, vec![], SemanticTerminatorKindV1::Return)],
            ),
            caller(3, true, 1),
        ],
        vec![function_id(0), function_id(2), function_id(3)],
    )
}

#[test]
fn canonical_evidence_roundtrip_is_source_allocation_independent() {
    let first_source = repeated();
    let first = roundtrip(&first_source);
    let second_source = repeated();
    let second = roundtrip(&second_source);
    assert_eq!(first, second);
}

#[test]
fn nested_and_mixed_roots_preserve_ancestry_and_passthrough_identity() {
    let source = admit(
        vec![caller(0, true, 1), caller(1, false, 2), leaf(2)],
        vec![function_id(0)],
    );
    let nested = roundtrip(&source);
    let instances = nested.roots()[0].instances();
    assert_eq!(instances[2].depth(), 2);
    assert_eq!(instances[2].parent(), Some(SemanticCallInstanceIdV1(1)));
    let source = mixed();
    let evidence = roundtrip(&source);
    assert!(!evidence.root(function_id(2)).unwrap().has_expanded_calls());
    assert_eq!(
        evidence
            .root(function_id(2))
            .unwrap()
            .expanded_function_identity(),
        source.functions()[2].identity()
    );
    let left = evidence.root(function_id(0)).unwrap();
    let right = evidence.root(function_id(3)).unwrap();
    assert_eq!(
        left.instances()[1].function_identity(),
        right.instances()[1].function_identity()
    );
    assert_ne!(left.identity(), right.identity());
}

#[test]
fn evidence_binds_limits_without_changing_existing_expansion_ids() {
    let source = repeated();
    let original = expand(&source);
    let tighter = SemanticCallExpansionV1::try_new(
        &source,
        SemanticCallExpansionLimitsV1 {
            depth: 1,
            ..SemanticCallExpansionLimitsV1::default()
        },
    )
    .unwrap();
    assert_eq!(original.identity(), tighter.identity());
    assert_eq!(
        original.roots()[0].identity(),
        tighter.roots()[0].identity()
    );
    let first = Evidence::from_checked_expansion(&source, &original).unwrap();
    let second = Evidence::from_checked_expansion(&source, &tighter).unwrap();
    assert_ne!(first.identity(), second.identity());
    assert_eq!(second.limits().depth, 1);
    assert_eq!(
        first.verify_against_checked_expansion(&source, &tighter),
        Err(EvidenceError::ReplayMismatch)
    );
    second
        .verify_against_checked_expansion(&source, &tighter)
        .unwrap();
}

// Offsets follow the wire schema, while all record cardinalities come from the
// actual checked view. Fixture changes cannot silently shift mutation targets.
struct BlockOffsets {
    start: usize,
    statements: Vec<usize>,
    terminator: usize,
}
struct RootOffsets {
    start: usize,
    end: usize,
    instances: Vec<usize>,
    locals: Vec<usize>,
    blocks: Vec<BlockOffsets>,
}

fn layout(expansion: &SemanticCallExpansionV1, bytes: &[u8]) -> Vec<RootOffsets> {
    let mut offset = 152;
    let mut roots = Vec::new();
    for root in expansion.roots() {
        let start = offset;
        offset += 116;
        let instances = root
            .instances()
            .iter()
            .map(|_| {
                let start = offset;
                offset += 64;
                start
            })
            .collect();
        let locals = root
            .local_origins()
            .iter()
            .map(|_| {
                let start = offset;
                offset += 44;
                start
            })
            .collect();
        let mut blocks = Vec::new();
        for block in root.block_origins() {
            let start = offset;
            offset += 48;
            let statements = block
                .statements()
                .iter()
                .map(|statement| {
                    let start = offset;
                    offset += match statement {
                        SemanticExpandedStatementOriginV1::Source { .. }
                        | SemanticExpandedStatementOriginV1::ReturnTransfer { .. } => 5,
                        _ => 9,
                    };
                    start
                })
                .collect();
            let terminator = offset;
            offset += if block.terminator() == SemanticExpandedTerminatorOriginV1::Source {
                1
            } else {
                5
            };
            blocks.push(BlockOffsets {
                start,
                statements,
                terminator,
            });
        }
        roots.push(RootOffsets {
            start,
            end: offset,
            instances,
            locals,
            blocks,
        });
    }
    assert_eq!(offset, bytes.len());
    roots
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn set_length(bytes: &mut [u8]) {
    let length = bytes.len() as u32;
    put_u32(bytes, 16, length);
}

#[test]
fn decoding_rejects_header_length_zero_identity_and_budget_corruption() {
    let source = repeated();
    let expansion = expand(&source);
    let evidence = Evidence::from_checked_expansion(&source, &expansion).unwrap();
    let original = evidence.canonical_bytes();
    let offsets = layout(&expansion, original);
    for offset in [0, 8, 10, 12] {
        let mut bytes = original.to_vec();
        bytes[offset] ^= 0x80;
        assert_eq!(
            Evidence::decode(&bytes).unwrap_err(),
            EvidenceError::InvalidHeader
        );
    }
    let root = &offsets[0];
    for offset in [
        20,
        52,
        root.start + 8,
        root.start + 40,
        root.start + 72,
        root.instances[1] + 4,
        root.locals[0] + 12,
        root.blocks[0].start + 12,
    ] {
        let mut bytes = original.to_vec();
        bytes[offset..offset + 32].fill(0);
        assert_eq!(
            Evidence::decode(&bytes).unwrap_err(),
            EvidenceError::InvalidIdentity
        );
    }
    for end in [0, 8, 151, original.len() - 1] {
        assert_eq!(
            Evidence::decode(&original[..end]).unwrap_err(),
            EvidenceError::InvalidLength
        );
    }
    let mut bytes = original.to_vec();
    bytes.push(0);
    set_length(&mut bytes);
    assert_eq!(
        Evidence::decode(&bytes).unwrap_err(),
        EvidenceError::InvalidLength
    );
    for offset in [
        148,
        root.start + 104,
        root.start + 108,
        root.start + 112,
        root.blocks[0].start + 44,
    ] {
        let mut bytes = original.to_vec();
        put_u32(&mut bytes, offset, u32::MAX);
        assert_eq!(
            Evidence::decode(&bytes).unwrap_err(),
            EvidenceError::InvalidLength
        );
    }
    for axis in 0..7 {
        let mut bytes = original.to_vec();
        bytes[84 + axis * 8..92 + axis * 8].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            Evidence::decode(&bytes).unwrap_err(),
            EvidenceError::InvalidLimits
        );
    }
    let mut bytes = original.to_vec();
    bytes[140..148].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        Evidence::decode(&bytes).unwrap_err(),
        EvidenceError::InvalidLimits
    );
}

#[test]
fn decoding_rejects_duplicate_reordered_roots_and_aggregate_budget_overrun() {
    let source = mixed();
    let expansion = expand(&source);
    let evidence = Evidence::from_checked_expansion(&source, &expansion).unwrap();
    let original = evidence.canonical_bytes();
    let roots = layout(&expansion, original);
    for order in [[1, 0, 2], [0, 0, 2]] {
        let mut bytes = original[..152].to_vec();
        for index in order {
            bytes.extend_from_slice(&original[roots[index].start..roots[index].end]);
        }
        set_length(&mut bytes);
        assert_eq!(
            Evidence::decode(&bytes).unwrap_err(),
            EvidenceError::NonCanonical
        );
    }
    let mut bytes = original.to_vec();
    let largest_root = expansion
        .roots()
        .iter()
        .map(|root| root.instances().len())
        .max()
        .unwrap();
    bytes[92..100].copy_from_slice(&(largest_root as u64).to_le_bytes());
    assert_eq!(
        Evidence::decode(&bytes).unwrap_err(),
        EvidenceError::InvalidLimits
    );
}

#[test]
fn decoding_rejects_frame_ancestry_ranges_and_duplicate_call_sites() {
    let source = repeated();
    let expansion = expand(&source);
    let evidence = Evidence::from_checked_expansion(&source, &expansion).unwrap();
    let original = evidence.canonical_bytes();
    let roots = layout(&expansion, original);
    let frame = roots[0].instances[1];
    for (offset, value) in [
        (frame + 36, 1),
        (frame + 44, 0),
        (frame + 52, 0),
        (frame + 60, 0),
        (frame, 0),
        (roots[0].instances[2] + 40, 0),
    ] {
        let mut bytes = original.to_vec();
        put_u32(&mut bytes, offset, value);
        assert!(matches!(
            Evidence::decode(&bytes),
            Err(EvidenceError::InvalidRecord | EvidenceError::NonCanonical)
        ));
    }
}

#[test]
fn decoding_rejects_origin_identity_order_tags_and_missing_frame_end() {
    let source = repeated();
    let expansion = expand(&source);
    let evidence = Evidence::from_checked_expansion(&source, &expansion).unwrap();
    let original = evidence.canonical_bytes();
    let roots = layout(&expansion, original);
    let root = &roots[0];
    for (first, second) in [
        (root.locals[0], root.locals[1]),
        (root.blocks[0].start, root.blocks[1].start),
    ] {
        let mut bytes = original.to_vec();
        bytes.copy_within(first + 12..first + 44, second + 12);
        assert_eq!(
            Evidence::decode(&bytes).unwrap_err(),
            EvidenceError::NonCanonical
        );
        let mut bytes = original.to_vec();
        put_u32(&mut bytes, second + 8, 0);
        assert_eq!(
            Evidence::decode(&bytes).unwrap_err(),
            EvidenceError::NonCanonical
        );
    }
    for offset in [root.blocks[0].statements[0], root.blocks[0].terminator] {
        let mut bytes = original.to_vec();
        bytes[offset] = u8::MAX;
        assert_eq!(
            Evidence::decode(&bytes).unwrap_err(),
            EvidenceError::InvalidRecord
        );
    }
    let mut checked_parameter = false;
    let mut checked_live = false;
    let mut checked_dead = false;
    for (block, offsets) in expansion.roots()[0]
        .block_origins()
        .iter()
        .zip(&root.blocks)
    {
        for (index, (origin, &offset)) in block
            .statements()
            .iter()
            .zip(&offsets.statements)
            .enumerate()
        {
            if matches!(origin, SemanticExpandedStatementOriginV1::FrameStorageLive { local, .. } if local.index() == 1)
            {
                let mut bytes = original.to_vec();
                put_u32(&mut bytes, offset + 5, 0);
                assert_eq!(
                    Evidence::decode(&bytes).unwrap_err(),
                    EvidenceError::NonCanonical
                );
                checked_live = true;
            }
            if matches!(
                origin,
                SemanticExpandedStatementOriginV1::ParameterTransfer { argument: 1, .. }
            ) {
                let mut bytes = original.to_vec();
                put_u32(&mut bytes, offset + 5, 0);
                assert_eq!(
                    Evidence::decode(&bytes).unwrap_err(),
                    EvidenceError::NonCanonical
                );
                checked_parameter = true;
            }
            if matches!(
                origin,
                SemanticExpandedStatementOriginV1::FrameStorageDead { .. }
            ) && index + 1 == block.statements().len()
            {
                let mut bytes = original.to_vec();
                drop(bytes.drain(offset..offset + 9));
                put_u32(
                    &mut bytes,
                    offsets.start + 44,
                    block.statements().len() as u32 - 1,
                );
                set_length(&mut bytes);
                assert_eq!(
                    Evidence::decode(&bytes).unwrap_err(),
                    EvidenceError::InvalidRecord
                );
                checked_dead = true;
            }
        }
    }
    assert!(checked_parameter && checked_live && checked_dead);
}

#[test]
fn inert_metadata_mutations_never_authenticate_from_matching_hash_fields() {
    let source = repeated();
    let expansion = expand(&source);
    let evidence = Evidence::from_checked_expansion(&source, &expansion).unwrap();
    let original = evidence.canonical_bytes();
    let roots = layout(&expansion, original);
    for offset in [
        roots[0].start + 72,
        roots[0].locals[0] + 12,
        roots[0].blocks[0].start + 12,
        140,
    ] {
        let mut bytes = original.to_vec();
        bytes[offset] ^= 0x40;
        let forged = Evidence::decode(&bytes).unwrap();
        assert_eq!(forged.expansion_identity(), evidence.expansion_identity());
        assert_eq!(forged.roots()[0].identity(), evidence.roots()[0].identity());
        assert_ne!(forged.identity(), evidence.identity());
        assert!(!forged.grants_proof_or_artifact_authority());
        assert_eq!(
            forged.verify_against_checked_expansion(&source, &expansion),
            Err(EvidenceError::ReplayMismatch)
        );
    }
}

#[test]
fn source_replay_rejects_deleted_parameter_even_when_inert_shape_is_valid() {
    let source = repeated();
    let expansion = expand(&source);
    let evidence = Evidence::from_checked_expansion(&source, &expansion).unwrap();
    let roots = layout(&expansion, evidence.canonical_bytes());
    let block = &expansion.roots()[0].block_origins()[0];
    let offsets = &roots[0].blocks[0];
    let index = block
        .statements()
        .iter()
        .position(|origin| {
            matches!(
                origin,
                SemanticExpandedStatementOriginV1::ParameterTransfer { argument: 1, .. }
            )
        })
        .unwrap();
    let mut bytes = evidence.canonical_bytes().to_vec();
    drop(bytes.drain(offsets.statements[index]..offsets.statements[index] + 9));
    put_u32(
        &mut bytes,
        offsets.start + 44,
        block.statements().len() as u32 - 1,
    );
    set_length(&mut bytes);
    let forged = Evidence::decode(&bytes).unwrap();
    assert_eq!(
        forged.verify_against_checked_expansion(&source, &expansion),
        Err(EvidenceError::ReplayMismatch)
    );
}

#[test]
fn construction_and_revalidation_reject_tampered_live_maps_and_other_source() {
    let source = repeated();
    let evidence = roundtrip(&source);
    for axis in 0..4 {
        let mut expansion = expand(&source);
        let root = &mut expansion.roots[0];
        match axis {
            0 => root.local_origins[0].local = local(1),
            1 => root.block_origins[0].terminator = SemanticExpandedTerminatorOriginV1::Source,
            2 => root.instances[1].call_block = Some(SemanticBlockIdV1::from_index(1)),
            _ => {
                root.block_origins[0].statements[0] =
                    SemanticExpandedStatementOriginV1::Source { statement: 0 }
            }
        }
        assert_eq!(
            Evidence::from_checked_expansion(&source, &expansion).unwrap_err(),
            EvidenceError::Expansion(SemanticCallExpansionErrorV1::ReplayMismatch)
        );
        assert_eq!(
            evidence.verify_against_checked_expansion(&source, &expansion),
            Err(EvidenceError::Expansion(
                SemanticCallExpansionErrorV1::ReplayMismatch
            ))
        );
    }
    let other = mixed();
    assert_eq!(
        evidence.verify_against_checked_expansion(&other, &expand(&other)),
        Err(EvidenceError::SourceMismatch)
    );
}

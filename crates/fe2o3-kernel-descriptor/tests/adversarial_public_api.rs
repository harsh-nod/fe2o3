use std::{
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
};

use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CANONICAL_CODE_OBJECT_DIGEST_OFFSET,
    CanonicalCodeObjectDigest, CapabilityV1, CodeObjectVersion, CompilerIdentityV1, DecodeError,
    DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DeviceTargetV1,
    DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1,
    KernelId, LaunchConstraintsV1, LogicalArgumentV1, MAX_DESCRIPTOR_TABLE_BYTES,
    ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
    ValidationError, decode_device_descriptor_table_v1, encode_device_descriptor_table_v1,
};

const CODE_OBJECT_DIGEST: [u8; 32] = [0xd3; 32];
const FIRST_KERNEL_ID: [u8; 32] = [0x21; 32];
const SECOND_KERNEL_ID: [u8; 32] = [0x42; 32];
const TARGET: &str = "gfx942:sramecc+:xnack-";

fn name(value: &str) -> ValidName {
    ValidName::new(value).expect("fixture name must be valid")
}

fn text(value: &str) -> Text {
    Text::new(value).expect("fixture text must be valid")
}

fn evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([identity; 32]),
        EvidenceDigest::from_sha256_bytes([digest; 32]),
    )
}

fn launch() -> LaunchConstraintsV1 {
    LaunchConstraintsV1::new(
        1,
        BlockSizeV1::AtMost(DimensionsV1::new(256, 1, 1).expect("valid block dimensions")),
        DimensionsV1::new(u32::MAX, 1, 1).expect("valid grid dimensions"),
        1024,
        1024,
        64 * 1024,
    )
    .expect("valid launch constraints")
}

fn representative_table() -> DeviceDescriptorTableV1 {
    let scalar_type = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
    let scalar_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
    let shared_type =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_type =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let disjoint_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));

    let full_kernel = KernelDescriptorV1::new(
        KernelId::from_bytes(FIRST_KERNEL_ID),
        name("transform"),
        name("transform_entry"),
        name("transform_descriptor"),
        evidence(0x51, 0x52),
        evidence(0x53, 0x54),
        vec![
            CapabilityV1::AmdWave,
            CapabilityV1::Subgroup,
            CapabilityV1::Atomics,
        ],
        KernelAbiLayoutV1::new(40, 80, 8).expect("valid full-kernel ABI"),
        launch(),
        vec![
            LogicalArgumentV1::scalar(0, name("scale"), &scalar_type, &scalar_layout, 0)
                .expect("valid scalar argument"),
            LogicalArgumentV1::shared_slice(1, name("source"), &shared_type, &shared_layout, 8)
                .expect("valid shared-slice argument"),
            LogicalArgumentV1::disjoint_slice(
                2,
                name("destination"),
                &disjoint_type,
                &disjoint_layout,
                AccessMode::ReadWrite,
                24,
            )
            .expect("valid DisjointSlice argument"),
        ],
    )
    .expect("valid full kernel");

    let scalar_kernel = KernelDescriptorV1::new(
        KernelId::from_bytes(SECOND_KERNEL_ID),
        name("scale_only"),
        name("scale_only_entry"),
        name("scale_only_descriptor"),
        evidence(0x61, 0x62),
        evidence(0x63, 0x64),
        vec![CapabilityV1::Shuffle, CapabilityV1::Subgroup],
        KernelAbiLayoutV1::new(4, 32, 8).expect("valid scalar-kernel ABI"),
        launch(),
        vec![
            LogicalArgumentV1::scalar(0, name("factor"), &scalar_type, &scalar_layout, 0)
                .expect("valid scalar argument"),
        ],
    )
    .expect("valid scalar kernel");

    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes(CODE_OBJECT_DIGEST),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            text("rustc-adversarial"),
            text("1.94.0-nightly-test"),
            [0xc1; 20],
        ),
        ProducerIdentityV1::new(text("cargo-fe2o3-tests"), text("0.1.0-test")),
        DeviceTargetV1::parse(TARGET).expect("canonical target"),
        vec![disjoint_type, scalar_type, shared_type],
        vec![shared_layout, disjoint_layout, scalar_layout],
        vec![scalar_kernel, full_kernel],
    )
    .expect("valid representative table")
}

fn encoded_fixture() -> Vec<u8> {
    encode_device_descriptor_table_v1(&representative_table()).expect("fixture must encode")
}

fn decode_without_panicking(bytes: &[u8]) -> Result<DeviceDescriptorTableV1, DecodeError> {
    catch_unwind(AssertUnwindSafe(|| {
        decode_device_descriptor_table_v1(bytes)
    }))
    .unwrap_or_else(|_| panic!("public decoder panicked for {} bytes", bytes.len()))
}

fn rejected(bytes: &[u8]) -> DecodeError {
    decode_without_panicking(bytes).expect_err("adversarial bytes must be rejected")
}

fn assert_canonical_when_accepted(bytes: &[u8]) {
    if let Ok(table) = decode_without_panicking(bytes) {
        assert_eq!(
            encode_device_descriptor_table_v1(&table).expect("accepted table must re-encode"),
            bytes,
            "accepted bytes were not canonical"
        );
    }
}

fn accepted_single_byte_mutation(
    encoded: &[u8],
    offset: usize,
) -> (Vec<u8>, DeviceDescriptorTableV1) {
    let mut mutated = encoded.to_vec();
    mutated[offset] ^= 1;
    let decoded = decode_without_panicking(&mutated).expect("mutation must remain valid");
    assert_eq!(
        encode_device_descriptor_table_v1(&decoded).expect("accepted table must re-encode"),
        mutated,
        "accepted mutation must re-encode exactly"
    );
    (mutated, decoded)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("u16 in fixture"),
    )
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[derive(Debug)]
struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn at(bytes: &'a [u8], position: usize) -> Self {
        Self { bytes, position }
    }

    fn skip(&mut self, count: usize) {
        self.position = self.position.checked_add(count).expect("fixture offset");
        assert!(
            self.position <= self.bytes.len(),
            "fixture wire is complete"
        );
    }

    fn text(&mut self) -> (usize, Range<usize>) {
        let length_offset = self.position;
        let length = usize::from(read_u16(self.bytes, length_offset));
        self.skip(2);
        (length_offset, self.range(length))
    }

    fn range(&mut self, count: usize) -> Range<usize> {
        let start = self.position;
        self.skip(count);
        start..self.position
    }
}

#[derive(Debug)]
struct TableOffsets {
    code_object_version: usize,
    pointer_width: usize,
    endianness: usize,
    header_reserved: usize,
    compiler_name_length: usize,
    compiler_commit: Range<usize>,
    target: Range<usize>,
    type_count: usize,
    layout_count: usize,
    kernel_count: usize,
    count_reserved: usize,
    type_records: usize,
    layout_records: usize,
    kernel_records: usize,
}

fn table_offsets(bytes: &[u8]) -> TableOffsets {
    let mut cursor = Cursor::at(bytes, 52);
    let (compiler_name_length, _) = cursor.text();
    cursor.text();
    let compiler_commit = cursor.range(20);
    cursor.text();
    cursor.text();
    let (_, target) = cursor.text();
    let type_count = cursor.position;
    let layout_count = type_count + 2;
    let kernel_count = layout_count + 2;
    let count_reserved = kernel_count + 2;
    let type_records = count_reserved + 2;
    let layout_records = type_records + usize::from(read_u16(bytes, type_count)) * 36;
    let kernel_records = layout_records + usize::from(read_u16(bytes, layout_count)) * 44;
    TableOffsets {
        code_object_version: 48,
        pointer_width: 49,
        endianness: 50,
        header_reserved: 51,
        compiler_name_length,
        compiler_commit,
        target,
        type_count,
        layout_count,
        kernel_count,
        count_reserved,
        type_records,
        layout_records,
        kernel_records,
    }
}

#[derive(Debug)]
struct ArgumentOffsets {
    source_index: usize,
    flags: usize,
    name_length: usize,
    ownership: usize,
    access: usize,
    alias: usize,
    reserved: usize,
    component_count: usize,
    component_reserved: usize,
    components: Vec<ComponentOffsets>,
}

#[derive(Debug)]
struct ComponentOffsets {
    kind: usize,
    scalar: usize,
    access: usize,
    alias: usize,
    flags_reserved: usize,
    reserved: usize,
}

#[derive(Debug)]
struct EvidenceOffsets {
    kind: usize,
    identity_scheme: usize,
    digest_algorithm: usize,
    reserved: usize,
    identity: Range<usize>,
    digest: Range<usize>,
}

#[derive(Debug)]
struct KernelOffsets {
    end: usize,
    kernel_id: Range<usize>,
    logical_name_length: usize,
    evidence: [EvidenceOffsets; 2],
    capability_count: usize,
    capability_tags: Vec<usize>,
    block_tag: usize,
    launch_reserved: usize,
    argument_count: usize,
    declared_component_count: usize,
    explicit_argument_size: usize,
    arguments: Vec<ArgumentOffsets>,
}

fn evidence_offsets(cursor: &mut Cursor<'_>) -> EvidenceOffsets {
    let kind = cursor.position;
    let identity_scheme = kind + 1;
    let digest_algorithm = kind + 2;
    let reserved = kind + 3;
    cursor.skip(4);
    let identity = cursor.range(32);
    let digest = cursor.range(32);
    EvidenceOffsets {
        kind,
        identity_scheme,
        digest_algorithm,
        reserved,
        identity,
        digest,
    }
}

fn kernel_offsets(bytes: &[u8], start: usize) -> KernelOffsets {
    let mut cursor = Cursor::at(bytes, start);
    let kernel_id = cursor.range(32);
    let (logical_name_length, _) = cursor.text();
    cursor.text();
    cursor.text();
    let evidence = [evidence_offsets(&mut cursor), evidence_offsets(&mut cursor)];

    let capability_count = cursor.position;
    let capabilities = usize::from(read_u16(bytes, capability_count));
    cursor.skip(2);
    let capability_tags = (0..capabilities)
        .map(|index| cursor.position + index * 2)
        .collect();
    cursor.skip(capabilities * 2);

    cursor.skip(1);
    let block_tag = cursor.position;
    cursor.skip(1);
    let launch_reserved = cursor.position;
    cursor.skip(2 + 12 + 12 + 4 + 4 + 4);

    let argument_count = cursor.position;
    let arguments = usize::from(read_u16(bytes, argument_count));
    cursor.skip(2);
    let declared_component_count = cursor.position;
    cursor.skip(2);
    let explicit_argument_size = cursor.position;
    cursor.skip(12);

    let mut argument_offsets = Vec::with_capacity(arguments);
    for _ in 0..arguments {
        let source_index = cursor.position;
        cursor.skip(2);
        let flags = cursor.position;
        cursor.skip(2);
        let (name_length, _) = cursor.text();
        cursor.skip(64);
        let ownership = cursor.position;
        let access = ownership + 1;
        let alias = ownership + 2;
        let reserved = ownership + 3;
        cursor.skip(4);
        let component_count = cursor.position;
        let components = usize::from(read_u16(bytes, component_count));
        cursor.skip(2);
        let component_reserved = cursor.position;
        cursor.skip(2);
        let mut component_offsets = Vec::with_capacity(components);
        for _ in 0..components {
            let kind = cursor.position;
            component_offsets.push(ComponentOffsets {
                kind,
                scalar: kind + 1,
                access: kind + 2,
                alias: kind + 3,
                flags_reserved: kind + 12,
                reserved: kind + 14,
            });
            cursor.skip(16);
        }
        argument_offsets.push(ArgumentOffsets {
            source_index,
            flags,
            name_length,
            ownership,
            access,
            alias,
            reserved,
            component_count,
            component_reserved,
            components: component_offsets,
        });
    }

    KernelOffsets {
        end: cursor.position,
        kernel_id,
        logical_name_length,
        evidence,
        capability_count,
        capability_tags,
        block_tag,
        launch_reserved,
        argument_count,
        declared_component_count,
        explicit_argument_size,
        arguments: argument_offsets,
    }
}

#[derive(Debug)]
struct WireOffsets {
    table: TableOffsets,
    kernels: Vec<KernelOffsets>,
}

fn wire_offsets(bytes: &[u8]) -> WireOffsets {
    let table = table_offsets(bytes);
    let mut start = table.kernel_records;
    let kernel_count = usize::from(read_u16(bytes, table.kernel_count));
    let mut kernels = Vec::with_capacity(kernel_count);
    for _ in 0..kernel_count {
        let kernel = kernel_offsets(bytes, start);
        start = kernel.end;
        kernels.push(kernel);
    }
    assert_eq!(start, bytes.len(), "fixture wire must be fully indexed");
    WireOffsets { table, kernels }
}

#[test]
fn representative_public_model_round_trips_canonically() {
    let table = representative_table();
    assert_eq!(table.kernels().len(), 2);
    assert_eq!(table.type_records().len(), 3);
    assert_eq!(table.layout_records().len(), 3);
    assert!(table.kernels()[0].arguments().iter().any(|arg| {
        arg.name().as_str() == "scale"
            && arg.physical_components().any(|(kind, _, _, _)| {
                matches!(
                    kind,
                    fe2o3_kernel_descriptor::PhysicalAbiComponentKind::ScalarByValue(_)
                )
            })
    }));
    assert!(
        table.kernels()[0]
            .arguments()
            .iter()
            .any(|arg| arg.name().as_str() == "source")
    );
    assert!(
        table.kernels()[0]
            .arguments()
            .iter()
            .any(|arg| arg.name().as_str() == "destination")
    );

    let encoded = encode_device_descriptor_table_v1(&table).expect("encode");
    let decoded = decode_without_panicking(&encoded).expect("canonical fixture must decode");
    assert_eq!(decoded, table);
    assert_eq!(
        encode_device_descriptor_table_v1(&decoded).expect("re-encode"),
        encoded
    );
}

#[test]
fn every_prefix_truncation_is_rejected_without_panicking() {
    let encoded = encoded_fixture();
    for length in 0..encoded.len() {
        assert!(
            decode_without_panicking(&encoded[..length]).is_err(),
            "decoder accepted prefix ending at byte {length}"
        );
    }
}

#[test]
fn deterministic_single_byte_mutations_never_panic_and_accept_only_canonical_bytes() {
    let encoded = encoded_fixture();
    for index in 0..encoded.len() {
        for replacement in [
            encoded[index] ^ 1,
            encoded[index] ^ 0x80,
            encoded[index] ^ 0xff,
        ] {
            let mut mutated = encoded.clone();
            mutated[index] = replacement;
            assert_canonical_when_accepted(&mutated);
        }
    }
}

#[test]
fn canonical_code_object_digest_slot_round_trips_each_mutated_byte() {
    let encoded = encoded_fixture();
    let digest_range = CANONICAL_CODE_OBJECT_DIGEST_OFFSET
        ..CANONICAL_CODE_OBJECT_DIGEST_OFFSET + CODE_OBJECT_DIGEST.len();
    assert_eq!(&encoded[digest_range.clone()], &CODE_OBJECT_DIGEST);

    for index in digest_range {
        let mut mutated = encoded.clone();
        mutated[index] ^= 0x5a;
        let decoded = decode_without_panicking(&mutated).expect("digest bytes are opaque data");
        let mut expected = CODE_OBJECT_DIGEST;
        expected[index - CANONICAL_CODE_OBJECT_DIGEST_OFFSET] ^= 0x5a;
        assert_eq!(decoded.canonical_code_object_digest().as_bytes(), &expected);
        assert_eq!(
            encode_device_descriptor_table_v1(&decoded).expect("re-encode mutated digest"),
            mutated
        );
    }
}

#[test]
fn opaque_compiler_evidence_and_kernel_id_bytes_accept_explicit_mutations() {
    let encoded = encoded_fixture();
    let offsets = wire_offsets(&encoded);
    let first_kernel = &offsets.kernels[0];
    assert_eq!(&encoded[first_kernel.kernel_id.clone()], &FIRST_KERNEL_ID);

    let compiler_commit = offsets.table.compiler_commit.clone();
    let (mutated, decoded) = accepted_single_byte_mutation(&encoded, compiler_commit.start + 7);
    assert_eq!(
        decoded.compiler().commit().as_slice(),
        &mutated[compiler_commit]
    );

    let source_identity = first_kernel.evidence[0].identity.clone();
    let (mutated, decoded) = accepted_single_byte_mutation(&encoded, source_identity.start + 5);
    assert_eq!(
        decoded.kernels()[0]
            .source_evidence()
            .identity()
            .as_bytes()
            .as_slice(),
        &mutated[source_identity]
    );

    let source_digest = first_kernel.evidence[0].digest.clone();
    let (mutated, decoded) = accepted_single_byte_mutation(&encoded, source_digest.start + 11);
    assert_eq!(
        decoded.kernels()[0]
            .source_evidence()
            .digest()
            .as_bytes()
            .as_slice(),
        &mutated[source_digest]
    );

    let executable_identity = first_kernel.evidence[1].identity.clone();
    let (mutated, decoded) =
        accepted_single_byte_mutation(&encoded, executable_identity.start + 13);
    assert_eq!(
        decoded.kernels()[0]
            .executable_ir_evidence()
            .identity()
            .as_bytes()
            .as_slice(),
        &mutated[executable_identity]
    );

    let executable_digest = first_kernel.evidence[1].digest.clone();
    let (mutated, decoded) = accepted_single_byte_mutation(&encoded, executable_digest.start + 17);
    assert_eq!(
        decoded.kernels()[0]
            .executable_ir_evidence()
            .digest()
            .as_bytes()
            .as_slice(),
        &mutated[executable_digest]
    );

    let kernel_id = first_kernel.kernel_id.clone();
    let (mutated, decoded) = accepted_single_byte_mutation(&encoded, kernel_id.end - 1);
    assert!(
        mutated[kernel_id.clone()] < mutated[offsets.kernels[1].kernel_id.clone()],
        "kernel ID mutation must preserve canonical ordering"
    );
    assert_eq!(
        decoded.kernels()[0].kernel_id().as_bytes().as_slice(),
        &mutated[kernel_id]
    );
}

#[test]
fn attacker_controlled_counts_and_lengths_are_rejected_by_schema_limits() {
    let encoded = encoded_fixture();
    let offsets = wire_offsets(&encoded);
    let table = &offsets.table;
    let kernel = &offsets.kernels[0];
    let first_argument = &kernel.arguments[0];

    for (offset, field) in [
        (table.type_count, "type records"),
        (table.layout_count, "layout records"),
        (table.kernel_count, "kernels"),
        (kernel.capability_count, "kernel capabilities"),
        (kernel.argument_count, "kernel arguments"),
        (kernel.declared_component_count, "physical ABI components"),
        (
            first_argument.component_count,
            "argument physical components",
        ),
    ] {
        let mut bomb = encoded.clone();
        write_u16(&mut bomb, offset, u16::MAX);
        assert!(
            matches!(
                rejected(&bomb),
                DecodeError::CountOutOfRange {
                    field: actual,
                    count,
                    ..
                } if actual == field && count == u64::from(u16::MAX)
            ),
            "wrong rejection for {field} count bomb"
        );
    }

    for (offset, field) in [
        (table.compiler_name_length, "compiler name"),
        (kernel.logical_name_length, "kernel logical name"),
        (first_argument.name_length, "argument name"),
    ] {
        let mut bomb = encoded.clone();
        write_u16(&mut bomb, offset, u16::MAX);
        assert!(
            matches!(
                rejected(&bomb),
                DecodeError::CountOutOfRange {
                    field: actual,
                    count,
                    ..
                } if actual == field && count == u64::from(u16::MAX)
            ),
            "wrong rejection for {field} length bomb"
        );
    }

    let mut declared_length_bomb = encoded.clone();
    write_u32(&mut declared_length_bomb, 12, u32::MAX);
    assert_eq!(rejected(&declared_length_bomb), DecodeError::Truncated);

    let mut explicit_size_bomb = encoded.clone();
    write_u32(
        &mut explicit_size_bomb,
        kernel.explicit_argument_size,
        u32::MAX,
    );
    assert!(matches!(
        rejected(&explicit_size_bomb),
        DecodeError::Validation(ValidationError::InvalidPhysicalAbi(_))
    ));

    let mut oversized = vec![0_u8; MAX_DESCRIPTOR_TABLE_BYTES + 1];
    oversized[..8].copy_from_slice(b"FE2O3KD\0");
    assert_eq!(
        rejected(&oversized),
        DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES
        }
    );
}

#[test]
fn trailing_bytes_reserved_fields_and_unknown_tags_are_rejected() {
    let encoded = encoded_fixture();
    let offsets = wire_offsets(&encoded);
    let table = &offsets.table;
    let kernel = &offsets.kernels[0];
    let argument = &kernel.arguments[1];
    let component = &argument.components[0];
    let scalar_component = &kernel.arguments[0].components[0];

    let mut trailing = encoded.clone();
    trailing.push(0xa5);
    assert_eq!(rejected(&trailing), DecodeError::TrailingBytes);

    let mut declared_trailing = trailing;
    let declared_length = u32::try_from(declared_trailing.len()).expect("bounded fixture length");
    write_u32(&mut declared_trailing, 12, declared_length);
    assert_eq!(rejected(&declared_trailing), DecodeError::TrailingBytes);

    let reserved_offsets = [
        table.header_reserved,
        table.count_reserved,
        table.type_records + 34,
        table.layout_records + 40,
        table.layout_records + 42,
        kernel.evidence[0].reserved,
        kernel.evidence[1].reserved,
        kernel.launch_reserved,
        argument.flags,
        argument.reserved,
        argument.component_reserved,
        component.scalar,
        component.flags_reserved,
        component.reserved,
    ];
    for offset in reserved_offsets {
        let mut nonzero_reserved = encoded.clone();
        nonzero_reserved[offset] = 1;
        assert!(
            matches!(
                rejected(&nonzero_reserved),
                DecodeError::NonzeroReserved { .. }
            ),
            "reserved byte {offset} did not fail as reserved"
        );
    }

    let mut unsupported_flags = encoded.clone();
    unsupported_flags[10] = 1;
    assert_eq!(
        rejected(&unsupported_flags),
        DecodeError::UnsupportedFlags(1)
    );

    let unknown_u8_tags = [
        table.code_object_version,
        table.pointer_width,
        table.endianness,
        table.type_records + 32,
        table.type_records + 33,
        table.layout_records + 32,
        kernel.evidence[0].kind,
        kernel.evidence[0].identity_scheme,
        kernel.evidence[0].digest_algorithm,
        kernel.evidence[1].kind,
        kernel.evidence[1].identity_scheme,
        kernel.evidence[1].digest_algorithm,
        kernel.block_tag,
        argument.ownership,
        argument.access,
        argument.alias,
        component.kind,
        scalar_component.scalar,
        scalar_component.access,
        scalar_component.alias,
    ];
    for offset in unknown_u8_tags {
        let mut unknown = encoded.clone();
        unknown[offset] = 0xff;
        assert!(
            matches!(rejected(&unknown), DecodeError::UnknownTag { .. }),
            "unknown byte tag at {offset} was not rejected as an unknown tag"
        );
    }

    for unknown_tag in [12, u16::MAX] {
        let mut unknown_capability = encoded.clone();
        write_u16(
            &mut unknown_capability,
            kernel.capability_tags[0],
            unknown_tag,
        );
        assert_eq!(
            rejected(&unknown_capability),
            DecodeError::UnknownTag {
                kind: "capability",
                tag: unknown_tag,
            }
        );
    }
}

#[test]
fn invalid_and_noncanonical_target_spellings_are_rejected() {
    let encoded = encoded_fixture();
    let target = wire_offsets(&encoded).table.target;
    assert_eq!(&encoded[target.clone()], TARGET.as_bytes());

    let mut invalid = encoded.clone();
    invalid[target.start] = b'G';
    assert!(matches!(
        rejected(&invalid),
        DecodeError::Validation(ValidationError::InvalidValue {
            field: "device target"
        })
    ));

    let reordered = b"gfx942:xnack-:sramecc+";
    assert_eq!(target.len(), reordered.len());
    let mut noncanonical = encoded;
    noncanonical[target].copy_from_slice(reordered);
    assert!(matches!(
        rejected(&noncanonical),
        DecodeError::Validation(ValidationError::NonCanonicalOrder {
            field: "device target features"
        })
    ));
}

#[test]
fn broken_sorted_and_unique_collections_are_rejected() {
    let encoded = encoded_fixture();
    let offsets = wire_offsets(&encoded);
    let table = &offsets.table;
    let first_kernel = &offsets.kernels[0];
    let second_kernel = &offsets.kernels[1];

    let mut reordered_types = encoded.clone();
    let first_type = reordered_types[table.type_records..table.type_records + 36].to_vec();
    let second_type = reordered_types[table.type_records + 36..table.type_records + 72].to_vec();
    reordered_types[table.type_records..table.type_records + 36].copy_from_slice(&second_type);
    reordered_types[table.type_records + 36..table.type_records + 72].copy_from_slice(&first_type);
    assert!(matches!(
        rejected(&reordered_types),
        DecodeError::Validation(ValidationError::NonCanonicalOrder {
            field: "type records"
        })
    ));

    let mut duplicate_layout = encoded.clone();
    let first_layout = duplicate_layout[table.layout_records..table.layout_records + 44].to_vec();
    duplicate_layout[table.layout_records + 44..table.layout_records + 88]
        .copy_from_slice(&first_layout);
    assert!(matches!(
        rejected(&duplicate_layout),
        DecodeError::Validation(ValidationError::Duplicate {
            field: "layout records"
        })
    ));

    let mut reordered_kernels = encoded.clone();
    reordered_kernels[first_kernel.kernel_id.clone()].fill(0xff);
    assert!(matches!(
        rejected(&reordered_kernels),
        DecodeError::Validation(ValidationError::NonCanonicalOrder { field: "kernels" })
    ));

    let mut duplicate_kernel = encoded.clone();
    duplicate_kernel[second_kernel.kernel_id.clone()].copy_from_slice(&FIRST_KERNEL_ID);
    assert!(matches!(
        rejected(&duplicate_kernel),
        DecodeError::Validation(ValidationError::Duplicate { field: "kernels" })
    ));

    let mut reordered_capabilities = encoded.clone();
    let first = first_kernel.capability_tags[0];
    let second = first_kernel.capability_tags[1];
    let first_tag = reordered_capabilities[first..first + 2].to_vec();
    let second_tag = reordered_capabilities[second..second + 2].to_vec();
    reordered_capabilities[first..first + 2].copy_from_slice(&second_tag);
    reordered_capabilities[second..second + 2].copy_from_slice(&first_tag);
    assert_eq!(rejected(&reordered_capabilities), DecodeError::NonCanonical);

    let mut duplicate_capability = encoded.clone();
    let first_tag = duplicate_capability[first..first + 2].to_vec();
    duplicate_capability[second..second + 2].copy_from_slice(&first_tag);
    assert!(matches!(
        rejected(&duplicate_capability),
        DecodeError::Validation(ValidationError::Duplicate {
            field: "kernel capability"
        })
    ));

    let mut broken_argument_indices = encoded;
    write_u16(
        &mut broken_argument_indices,
        first_kernel.arguments[1].source_index,
        0,
    );
    assert!(matches!(
        rejected(&broken_argument_indices),
        DecodeError::Validation(ValidationError::InvalidArgument(_))
    ));
}

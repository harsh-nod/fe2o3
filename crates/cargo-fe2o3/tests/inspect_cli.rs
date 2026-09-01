use std::env;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifacts::{
    AbiLayout, ArtifactContainerV1, BlockSize, BundleIndexV1, CodeObjectFormat, CodeObjectIdentity,
    CodeObjectPayload, CompilerIdentity, DigestAlgorithm, DigestBytes, Dimensions, Endianness,
    IdentityText, KernelEntry, LaunchContract, ManifestV1, Name, PointerWidth, TargetIdentity,
    ToolIdentity,
};
use fe2o3_source_isa_observation::characteristic_agent_v1::{
    AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1, AgentSourceIsaCharacteristicQueryV1,
    AgentSourceIsaCharacteristicRequestV1, AgentSourceIsaCharacteristicTargetQueryV1,
    encode_agent_source_isa_characteristic_request_line_v1,
};
use fe2o3_source_isa_observation::characteristic_v1::{
    MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1, SourceIsaCharacteristicBindingV1,
    SourceIsaCharacteristicCollectionV1, SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
    SourceIsaCharacteristicContentIdentityV1, SourceIsaCharacteristicIsaIntervalV1,
    SourceIsaCharacteristicKindV1, SourceIsaCharacteristicKirCoordinateV1,
    SourceIsaCharacteristicKirVersionV1, SourceIsaCharacteristicMemoryFormV1,
    SourceIsaCharacteristicMirCoordinateV1, SourceIsaCharacteristicRecordKindV1,
    SourceIsaCharacteristicScanStateV1, SourceIsaCharacteristicScanSummaryV1,
    SourceIsaCharacteristicSourceCoordinateV1, SourceIsaCharacteristicSourceSpanV1,
    SourceIsaCharacteristicStructuralCountsV1, SourceIsaCharacteristicTargetCorrelationV1,
    SourceIsaCharacteristicTargetProfileV1, SourceIsaCharacteristicTargetV1,
    SourceIsaCharacteristicTransformationV1,
    source_isa_characteristic_target_correlation_match_identity_v1,
};
use sha2::{Digest, Sha256};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn with_bytes(label: &str, bytes: &[u8]) -> Self {
        loop {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "cargo-fe2o3-inspect-{label}-{}-{id}.bin",
                process::id()
            ));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    file.write_all(bytes).expect("write inspect fixture");
                    return Self(path);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create inspect fixture: {error}"),
            }
        }
    }

    fn with_len(label: &str, length: u64) -> Self {
        let file = Self::with_bytes(label, &[]);
        fs::OpenOptions::new()
            .write(true)
            .open(&file.0)
            .expect("reopen inspect fixture")
            .set_len(length)
            .expect("size inspect fixture");
        file
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).expect("valid identity text")
}

fn name(value: &str) -> Name {
    Name::new(value).expect("valid name")
}

fn characteristic_id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn characteristic_content(byte: u8, length: u64) -> SourceIsaCharacteristicContentIdentityV1 {
    SourceIsaCharacteristicContentIdentityV1::new(characteristic_id(byte), length)
        .expect("valid characteristic content identity")
}

fn characteristic_collection(
    profile: SourceIsaCharacteristicTargetProfileV1,
) -> SourceIsaCharacteristicCollectionV1 {
    let target_kir =
        SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 0).expect("valid target-KIR coordinate");
    let source = SourceIsaCharacteristicSourceCoordinateV1::new(
        characteristic_id(20),
        SourceIsaCharacteristicSourceSpanV1::new(characteristic_id(21), 8, 12, 3, 5)
            .expect("valid source span"),
    )
    .expect("valid source coordinate");
    let interval =
        SourceIsaCharacteristicIsaIntervalV1::new(0, 0x40, 0x44).expect("valid ISA interval");
    let correlation = |catalog_record_ordinal| {
        SourceIsaCharacteristicTargetCorrelationV1::new(
            catalog_record_ordinal,
            SourceIsaCharacteristicRecordKindV1::SourceAnchored,
            Some(source),
            Some(characteristic_id(22)),
            Some(
                SourceIsaCharacteristicMirCoordinateV1::new(0, 0, 0).expect("valid MIR coordinate"),
            ),
            Some(characteristic_id(23)),
            Some(target_kir),
            target_kir,
            characteristic_id(24),
            SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 0, 0)
                .expect("valid compiler-handoff LLVM coordinate"),
            vec![interval, interval],
            SourceIsaCharacteristicTransformationV1::Duplicated,
        )
        .expect("valid characteristic correlation")
    };
    SourceIsaCharacteristicCollectionV1::new(
        SourceIsaCharacteristicBindingV1::new(
            profile,
            SourceIsaCharacteristicKirVersionV1::V8,
            characteristic_id(1),
            SourceIsaCharacteristicStructuralCountsV1 {
                functions: 1,
                defined_bodies: 1,
                blocks: 1,
                operations: 2,
            },
            characteristic_content(2, 20),
            characteristic_content(3, 30),
            characteristic_content(4, 40),
            characteristic_content(5, 50),
            characteristic_content(6, 60),
            characteristic_content(7, 70),
            characteristic_id(8),
            characteristic_id(9),
        )
        .expect("valid characteristic binding"),
        SourceIsaCharacteristicScanSummaryV1::new(
            2,
            2,
            2,
            2,
            2,
            2,
            0,
            2,
            SourceIsaCharacteristicScanStateV1::Complete,
        )
        .expect("valid scan summary"),
        vec![
            SourceIsaCharacteristicTargetV1::new(
                SourceIsaCharacteristicKindV1::GlobalStore {
                    form: SourceIsaCharacteristicMemoryFormV1::Plain,
                },
                target_kir,
                vec![correlation(0), correlation(1)],
            )
            .expect("valid characteristic target"),
            SourceIsaCharacteristicTargetV1::new(
                SourceIsaCharacteristicKindV1::GlobalStore {
                    form: SourceIsaCharacteristicMemoryFormV1::Guarded,
                },
                SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 1)
                    .expect("valid structural-only target-KIR coordinate"),
                Vec::new(),
            )
            .expect("valid structural-only characteristic target"),
        ],
        Vec::new(),
    )
    .expect("valid characteristic collection")
}

fn lowercase_hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_lowercase_hex_fixture(encoded: &str) -> Vec<u8> {
    let encoded = encoded
        .strip_suffix('\n')
        .expect("hex fixture ends in exactly one newline");
    assert_eq!(encoded.len() % 2, 0);
    assert!(
        encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |value: u8| match value {
                b'0'..=b'9' => value - b'0',
                b'a'..=b'f' => value - b'a' + 10,
                _ => unreachable!("validated lowercase hexadecimal"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn run_characteristic_agent(path: &Path, input: &[u8]) -> process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            "inspect",
            "--format=source-isa-characteristic-v1",
            "--output=agent-json-v1",
        ])
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn characteristic inspect service");
    child
        .stdin
        .take()
        .expect("characteristic service stdin")
        .write_all(input)
        .expect("write characteristic requests");
    child.wait_with_output().expect("wait for inspect service")
}

fn manifest(payload: &[u8]) -> ManifestV1 {
    let object_digest = DigestAlgorithm::Sha256.calculate(payload).bytes();
    let target = TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text("gfx1151"),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![],
    )
    .expect("valid target");
    let object = CodeObjectIdentity::new(
        object_digest,
        CodeObjectFormat::NativeExecutable,
        payload.len() as u64,
    )
    .expect("valid object");
    let launch = LaunchContract::new(
        1,
        BlockSize::Any,
        Dimensions::new(65_535, 1, 1).expect("valid grid"),
        0,
        0,
    )
    .expect("valid launch");
    let abi = AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).expect("valid empty ABI");
    let kernel = KernelEntry::new(
        DigestBytes::from_bytes([0x11; 32]),
        name("fixture"),
        name("fixture.kd"),
        DigestBytes::from_bytes([0x22; 32]),
        DigestBytes::from_bytes([0x33; 32]),
        object_digest,
        vec![],
        launch,
        abi,
    )
    .expect("valid kernel");
    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target,
        vec![object],
        vec![kernel],
    )
    .expect("valid manifest")
}

fn run_inspect(path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args(["inspect", path.to_str().expect("UTF-8 fixture path")])
        .output()
        .expect("run inspect")
}

fn source_isa_collection() -> Vec<u8> {
    const HEADER_BYTES: usize = 80;
    const TOTAL_BYTES: usize = HEADER_BYTES + 32 + 32;
    const DOMAIN: &[u8] = b"FE2O3/SOURCE-ISA-OBSERVATION-COLLECTION/V1\0";
    let mut bytes = Vec::with_capacity(TOTAL_BYTES);
    bytes.extend_from_slice(b"F2SICOL1");
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
    bytes.extend_from_slice(&(TOTAL_BYTES as u32).to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&6_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&[0x41; 32]);
    bytes.extend_from_slice(&[0x42; 16]);
    bytes.extend_from_slice(&[0x43; 32]);
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update(&bytes);
    bytes.extend_from_slice(&digest.finalize());
    assert_eq!(bytes.len(), TOTAL_BYTES);
    bytes
}

#[test]
fn auto_inspects_manifest_container_and_bundle_fixtures() {
    let payload = b"not-an-executable-fixture";
    let manifest = manifest(payload);
    let container = ArtifactContainerV1::new(
        manifest.clone(),
        DigestAlgorithm::Sha256,
        vec![
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, payload.to_vec())
                .expect("valid payload"),
        ],
    )
    .expect("valid container");
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container))
        .expect("valid bundle index");

    for (label, bytes, expected) in [
        ("manifest", manifest.to_bytes(), "format: fe2o3-manifest-v1"),
        (
            "container",
            container.to_bytes(),
            "format: fe2o3-container-v1",
        ),
        ("bundle", bundle.to_bytes(), "format: fe2o3-bundle-index-v1"),
    ] {
        let fixture = TempFile::with_bytes(label, &bytes);
        let output = run_inspect(&fixture.0);
        assert!(
            output.status.success(),
            "{label} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 inspect output");
        assert!(stdout.contains(expected), "{label} output: {stdout}");
        assert!(stdout.contains("authority: descriptive-only"));
        assert!(stdout.contains("architecture=gfx1151"));
        assert!(stdout.contains("symbol=fixture.kd"));
        if label == "container" {
            assert!(stdout.contains("abi-fields=0\ndigest-algorithm: sha256"));
        }
    }
}

#[test]
fn malformed_and_unknown_inputs_fail_without_execution() {
    let malformed = TempFile::with_bytes("malformed", b"FE2O3AM\0");
    let output = run_inspect(&malformed.0);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid fe2o3 manifest"));

    let unknown = TempFile::with_bytes("unknown", b"plain text is not an artifact");
    let output = run_inspect(&unknown.0);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized input magic"));
}

#[test]
fn explicit_format_mismatch_fails_closed() {
    let fixture = TempFile::with_bytes("mismatch", &manifest(b"payload").to_bytes());
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            "inspect",
            "--format=hsaco",
            fixture.0.to_str().expect("UTF-8 fixture path"),
        ])
        .output()
        .expect("run inspect");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid HSACO"));
}

#[test]
fn source_isa_collection_cli_supports_auto_explicit_and_hostile_inputs() {
    let bytes = source_isa_collection();
    let fixture = TempFile::with_bytes("source-isa", &bytes);
    let expected_human = format!(
        concat!(
            "format: fe2o3-source-isa-observation-collection-v1\n",
            "authority: observation-only\n",
            "compiler-authority: false\n",
            "proof-authority: false\n",
            "artifact-authority: false\n",
            "runtime-authority: false\n",
            "hardware-execution-observed: false\n",
            "complete-machine-coverage-proved: false\n",
            "semantic-refinement-proved: false\n",
            "configuration: {}\n",
            "session: {}\n",
            "frames: 0\n",
            "missing-units: 1\n",
            "transport-failure: 6\n",
            "missing-unit[0]: {}\n"
        ),
        "41".repeat(32),
        "42".repeat(16),
        "43".repeat(32),
    );
    for extra in [None, Some("--format=source-isa-observation")] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
        command.arg("inspect");
        if let Some(extra) = extra {
            command.arg(extra);
        }
        let output = command
            .arg(&fixture.0)
            .output()
            .expect("run source/ISA inspect");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 inspect output");
        assert_eq!(stdout, expected_human);
    }

    let agent = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            "inspect",
            "--format=source-isa-observation",
            "--output=agent-json-v1",
        ])
        .arg(&fixture.0)
        .output()
        .expect("run typed source/ISA inspect");
    assert!(
        agent.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&agent.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&agent.stdout).expect("typed JSON");
    assert_eq!(response["status"], "ok");
    assert_eq!(response["request_id"], 1);
    assert_eq!(response["response_revision"], 1);
    assert_eq!(response["result"]["result"], "collection_page");
    assert_eq!(response["result"]["page"]["page_exhausted"], true);
    assert_eq!(response["result"]["authority"]["runtime_authority"], false);

    let mut discovery = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args(["inspect", "--output=agent-json-v1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn source/ISA discovery");
    discovery
        .stdin
        .take()
        .expect("discovery stdin")
        .write_all(
            concat!(
                "{\"operation\":\"discover_capabilities\",\"schema\":\"fe2o3-agent-source-isa-request-v1\",\"request_id\":9}\n",
                "{\"operation\":\"discover_capabilities\",\"schema\":\"fe2o3-agent-source-isa-request-v1\",\"request_id\":10}\n",
                "{\"operation\":\"discover_capabilities\",\"schema\":\"fe2o3-agent-source-isa-request-v1\",\"request_id\":10}\n"
            )
            .as_bytes(),
        )
        .expect("write discovery request");
    let discovery = discovery.wait_with_output().expect("wait for discovery");
    assert!(discovery.status.success());
    let responses = discovery
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice::<serde_json::Value>(line).expect("discovery JSON"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["request_id"], 9);
    assert_eq!(responses[0]["response_revision"], 1);
    assert_eq!(responses[0]["result"]["result"], "capabilities");
    assert_eq!(responses[1]["request_id"], 10);
    assert_eq!(responses[1]["response_revision"], 2);
    assert_eq!(responses[2]["request_id"], 10);
    assert_eq!(responses[2]["response_revision"], 3);
    assert_eq!(responses[2]["error"], "duplicate_request_id");

    let mut trailing = bytes;
    trailing.push(0);
    let hostile = TempFile::with_bytes("source-isa-trailing", &trailing);
    let output = run_inspect(&hostile.0);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("invalid source/ISA observation collection")
    );

    let typed_hostile = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            "inspect",
            "--format=source-isa-observation",
            "--output=agent-json-v1",
        ])
        .arg(&hostile.0)
        .output()
        .expect("run typed hostile source/ISA inspect");
    assert!(typed_hostile.status.success());
    assert!(typed_hostile.stderr.is_empty());
    let typed_hostile: serde_json::Value =
        serde_json::from_slice(&typed_hostile.stdout).expect("typed hostile JSON");
    assert_eq!(typed_hostile["status"], "error");
    assert_eq!(typed_hostile["request_id"], 1);
    assert_eq!(typed_hostile["operation"], "inspect_source_isa_collection");
    assert_eq!(typed_hostile["error"], "invalid_collection");
}

#[test]
fn characteristic_archive_cli_serves_all_bounded_query_planes() {
    let collection = characteristic_collection(SourceIsaCharacteristicTargetProfileV1::Gfx942);
    let collection_identity = lowercase_hex(collection.identity());
    let occurrence_identity = lowercase_hex(
        source_isa_characteristic_target_correlation_match_identity_v1(
            collection.identity(),
            collection.targets()[0].identity(),
            0,
        ),
    );
    let encoded_collection = collection.encode_canonical().expect("encode collection");
    let archive_fixture = include_str!("fixtures/source_isa_characteristic_cli_v1/collection.hex");
    assert_eq!(
        format!(
            "{}\n",
            encoded_collection
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ),
        archive_fixture
    );
    let archive_bytes = decode_lowercase_hex_fixture(archive_fixture);
    let fixture = TempFile::with_bytes("source-isa-characteristic", &archive_bytes);
    let requests = [
        AgentSourceIsaCharacteristicRequestV1::DiscoverCapabilities {
            schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 1,
        },
        AgentSourceIsaCharacteristicRequestV1::QueryTargets {
            schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 2,
            collection_identity: collection_identity.clone(),
            query: AgentSourceIsaCharacteristicTargetQueryV1::All,
            cursor: None,
            limit: 64,
        },
        AgentSourceIsaCharacteristicRequestV1::QueryFacts {
            schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 3,
            collection_identity: collection_identity.clone(),
            query: AgentSourceIsaCharacteristicQueryV1::All,
            cursor: None,
            limit: 64,
        },
        AgentSourceIsaCharacteristicRequestV1::QueryIntervals {
            schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 4,
            collection_identity,
            occurrence_identity,
            cursor: None,
            limit: 64,
        },
    ]
    .iter()
    .map(|request| {
        encode_agent_source_isa_characteristic_request_line_v1(request)
            .expect("encode canonical request")
    })
    .collect::<String>();
    assert_eq!(
        requests,
        include_str!("fixtures/source_isa_characteristic_cli_v1/requests.jsonl")
    );

    let output = run_characteristic_agent(&fixture.0, requests.as_bytes());
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        include_bytes!("fixtures/source_isa_characteristic_cli_v1/responses.jsonl")
    );
    let responses = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice::<serde_json::Value>(line).expect("response JSON"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 4);
    assert_eq!(responses[0]["result"]["result"], "capabilities");
    assert_eq!(responses[1]["result"]["result"], "target_page");
    assert_eq!(responses[2]["result"]["result"], "fact_page");
    assert_eq!(responses[3]["result"]["result"], "interval_page");
    for (index, response) in responses.iter().enumerate() {
        assert_eq!(response["request_id"], u64::try_from(index + 1).unwrap());
        assert_eq!(
            response["response_revision"],
            u64::try_from(index + 1).unwrap()
        );
        let authority = &response["result"]["authority"];
        assert_eq!(authority["observation_only"], true);
        assert_eq!(
            authority["service_provenance"],
            "canonical_self_claimed_archive"
        );
        assert_eq!(authority["canonical_self_claimed_archive"], true);
        assert_eq!(authority["archive_authenticity_proved"], false);
        assert_eq!(authority["producer_evidence_authenticated"], false);
        assert_eq!(authority["compiler_authority"], false);
        assert_eq!(authority["runtime_authority"], false);
        assert_eq!(authority["hardware_observation_authority"], false);
    }
    let summary = &responses[0]["result"]["collection"];
    assert_eq!(summary["target_count"], 2);
    assert_eq!(summary["scan"]["catalog_record_count"], 2);
    assert_eq!(summary["scan"]["retained_target_correlation_count"], 2);

    let targets = responses[1]["result"]["page"]["targets"]
        .as_array()
        .expect("target array");
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0]["kind"]["memory_form"]["label"], "plain");
    assert_eq!(targets[0]["correlation_count"], 2);
    assert_eq!(targets[1]["kind"]["memory_form"]["label"], "guarded");
    assert_eq!(targets[1]["correlation_count"], 0);

    let facts = responses[2]["result"]["page"]["facts"]
        .as_array()
        .expect("fact array");
    assert_eq!(facts.len(), 2);
    assert_ne!(
        facts[0]["occurrence_identity"],
        facts[1]["occurrence_identity"]
    );
    assert_eq!(
        facts[0]["outcome"]["correlation"]["catalog_record_ordinal"],
        0
    );
    assert_eq!(
        facts[1]["outcome"]["correlation"]["catalog_record_ordinal"],
        1
    );
    assert_eq!(
        facts[0]["outcome"]["correlation"]["source"],
        facts[1]["outcome"]["correlation"]["source"]
    );
    assert_eq!(facts[0]["outcome"]["correlation"]["interval_count"], 2);
    assert_eq!(
        facts[0]["outcome"]["correlation"]["transformation"]["label"],
        "duplicated"
    );

    let intervals = responses[3]["result"]["page"]["intervals"]
        .as_array()
        .expect("interval array");
    assert_eq!(intervals.len(), 2);
    assert_eq!(intervals[0]["ordinal"], 0);
    assert_eq!(intervals[1]["ordinal"], 1);
    assert_ne!(intervals[0]["identity"], intervals[1]["identity"]);
    assert_eq!(intervals[0]["interval"], intervals[1]["interval"]);
}

#[test]
fn characteristic_archive_cli_rejects_hostile_files_and_requests() {
    let collection = characteristic_collection(SourceIsaCharacteristicTargetProfileV1::Gfx942);
    let bytes = collection.encode_canonical().expect("encode collection");
    let fixture = TempFile::with_bytes("source-isa-characteristic-hostile", &bytes);

    let human = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args(["inspect", "--format=source-isa-characteristic-v1"])
        .arg(&fixture.0)
        .output()
        .expect("run human characteristic inspect");
    assert!(!human.status.success());
    assert!(String::from_utf8_lossy(&human.stderr).contains("agent-json-v1 service"));

    let auto = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args(["inspect", "--output=agent-json-v1"])
        .arg(&fixture.0)
        .output()
        .expect("run auto characteristic inspect");
    assert!(!auto.status.success());
    assert!(String::from_utf8_lossy(&auto.stderr).contains(
        "requires --format source-isa-observation or --format source-isa-characteristic-v1"
    ));

    let mut trailing = bytes.clone();
    trailing.push(0);
    let trailing = TempFile::with_bytes("source-isa-characteristic-trailing", &trailing);
    let trailing = run_characteristic_agent(&trailing.0, &[]);
    assert!(!trailing.status.success());
    assert!(
        String::from_utf8_lossy(&trailing.stderr)
            .contains("failed to validate canonical self-claimed characteristic archive")
    );

    let mut malformed = bytes;
    malformed[0] ^= 1;
    let malformed = TempFile::with_bytes("source-isa-characteristic-malformed", &malformed);
    let malformed = run_characteristic_agent(&malformed.0, &[]);
    assert!(!malformed.status.success());
    assert!(
        String::from_utf8_lossy(&malformed.stderr)
            .contains("failed to validate canonical self-claimed characteristic archive")
    );

    let oversized = TempFile::with_len(
        "source-isa-characteristic-oversized",
        u64::try_from(MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1).unwrap() + 1,
    );
    let oversized = run_characteristic_agent(&oversized.0, &[]);
    assert!(!oversized.status.success());
    assert!(
        String::from_utf8_lossy(&oversized.stderr).contains("must be a non-symlink regular file")
    );

    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt as _;
        use std::os::unix::fs::symlink;
        use std::thread;
        use std::time::{Duration, Instant};

        let fifo = env::temp_dir().join(format!(
            "cargo-fe2o3-inspect-characteristic-fifo-{}-{}",
            process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let fifo_name = CString::new(fifo.as_os_str().as_bytes()).expect("FIFO path has no NUL");
        // SAFETY: `fifo_name` is a live NUL-terminated path and the mode is a valid `mode_t`.
        assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
        let unblock_path = fifo.clone();
        let unblock = thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            let _ = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(unblock_path);
        });
        let started = Instant::now();
        let fifo_output = run_characteristic_agent(&fifo, &[]);
        let elapsed = started.elapsed();
        unblock.join().expect("join FIFO unblock safeguard");
        let _ = fs::remove_file(&fifo);
        assert!(!fifo_output.status.success());
        assert!(
            elapsed < Duration::from_secs(1),
            "FIFO open blocked for {elapsed:?}"
        );
        assert!(
            String::from_utf8_lossy(&fifo_output.stderr)
                .contains("must be a non-symlink regular file")
        );

        let link = fixture.0.with_extension("symlink");
        let _ = fs::remove_file(&link);
        symlink(&fixture.0, &link).expect("create hostile symlink");
        let linked = run_characteristic_agent(&link, &[]);
        let _ = fs::remove_file(&link);
        assert!(!linked.status.success());
        assert!(String::from_utf8_lossy(&linked.stderr).contains("without following symlinks"));
    }

    let wrong_identity = AgentSourceIsaCharacteristicRequestV1::QueryTargets {
        schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 10,
        collection_identity: "aa".repeat(32),
        query: AgentSourceIsaCharacteristicTargetQueryV1::All,
        cursor: None,
        limit: 1,
    };
    let mut hostile_requests =
        encode_agent_source_isa_characteristic_request_line_v1(&wrong_identity)
            .expect("encode wrong-identity request");
    hostile_requests.push_str(
        "{\"operation\":\"discover_capabilities\",\"schema\":\"fe2o3-agent-source-isa-characteristic-request-v1\",\"request_id\":11,\"collection_bytes\":\"00\"}\n",
    );
    let output = run_characteristic_agent(&fixture.0, hostile_requests.as_bytes());
    assert!(output.status.success());
    let responses = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice::<serde_json::Value>(line).expect("response JSON"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["error"], "collection_identity_mismatch");
    assert_eq!(responses[1]["error"], "invalid_request");
    assert_eq!(responses[1]["terminal"], false);
}

#[test]
fn resealed_characteristic_substitution_remains_an_unauthenticated_archive() {
    let original = characteristic_collection(SourceIsaCharacteristicTargetProfileV1::Gfx942);
    let substituted = characteristic_collection(SourceIsaCharacteristicTargetProfileV1::Gfx950);
    assert_ne!(original.identity(), substituted.identity());
    let fixture = TempFile::with_bytes(
        "source-isa-characteristic-resealed-substitution",
        &substituted.encode_canonical().expect("encode substitution"),
    );
    let request = AgentSourceIsaCharacteristicRequestV1::DiscoverCapabilities {
        schema: AGENT_SOURCE_ISA_CHARACTERISTIC_REQUEST_SCHEMA_V1.to_owned(),
        request_id: 20,
    };
    let request =
        encode_agent_source_isa_characteristic_request_line_v1(&request).expect("encode discovery");
    let output = run_characteristic_agent(&fixture.0, request.as_bytes());
    assert!(output.status.success());
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("discovery response");
    assert_eq!(
        response["result"]["collection"]["identity"],
        lowercase_hex(substituted.identity())
    );
    let authority = &response["result"]["authority"];
    assert_eq!(
        authority["service_provenance"],
        "canonical_self_claimed_archive"
    );
    assert_eq!(authority["canonical_self_claimed_archive"], true);
    assert_eq!(authority["archive_authenticity_proved"], false);
    assert_eq!(authority["producer_evidence_authenticated"], false);
}

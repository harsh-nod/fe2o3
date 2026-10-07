//! Bounded diagnostic files from the actual model consumer, never proof receipts.
use super::*;
use std::io::{self, Read, Write};

const INPUT_LIMIT: usize = 64 * 1024;
const RECORD_LIMIT: usize = 16 * 1024;

struct Arguments(Vec<String>);

impl<'de> Deserialize<'de> for Arguments {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ArgumentsVisitor;
        impl<'de> serde::de::Visitor<'de> for ArgumentsVisitor {
            type Value = Arguments;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("at most 128 rustc arguments")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Arguments, A::Error> {
                let mut args = Vec::with_capacity(128);
                while let Some(argument) = sequence.next_element::<String>()? {
                    if args.len() == 128 {
                        return Err(serde::de::Error::custom("too many model export arguments"));
                    }
                    args.push(argument);
                }
                Ok(Arguments(args))
            }
        }
        deserializer.deserialize_seq(ArgumentsVisitor)
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn bounded_input(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > INPUT_LIMIT as u64 {
        return Err(invalid("model export input is not a bounded regular file"));
    }
    let mut bytes = Vec::with_capacity(INPUT_LIMIT + 1);
    std::fs::File::open(path)?
        .take((INPUT_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > INPUT_LIMIT || bytes.len() as u64 != metadata.len() {
        return Err(invalid("model export input size changed"));
    }
    Ok(bytes)
}

fn file_stem(path: &Path) -> io::Result<&str> {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| invalid("model export filename is not UTF-8"))?;
    if stem.is_empty()
        || stem.len() > 96
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(invalid("model export filename is not a flat bounded name"));
    }
    Ok(stem)
}

fn create_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn flag<'a>(args: &'a [String], prefix: &str) -> io::Result<&'a str> {
    let mut values = args
        .iter()
        .filter_map(|argument| argument.strip_prefix(prefix));
    let value = values
        .next()
        .ok_or_else(|| invalid("missing model export compiler flag"))?;
    if values.next().is_some() {
        return Err(invalid("duplicate model export compiler flag"));
    }
    Ok(value)
}

#[derive(Serialize)]
struct Record<'a> {
    format: &'static str,
    qualification: bool,
    target: &'a str,
    codegen_opt_level: u8,
    mir_opt_level: u8,
    case: &'a str,
    request_sha256: [u8; 32],
    rust_source_sha256: [u8; 32],
    request_file: String,
    source_file: String,
    model_file: String,
    model_bytes: usize,
    observation: &'a ModelObservation,
}

fn export(
    directory: &Path,
    request: &Path,
    response: &Path,
    source: &Path,
    bytes: &[u8],
    observation: &ModelObservation,
) -> io::Result<()> {
    if !directory.is_absolute()
        || !std::fs::symlink_metadata(directory)?.is_dir()
        || directory.canonicalize()? != directory
    {
        return Err(invalid(
            "model export requires an existing canonical directory",
        ));
    }
    if bytes.is_empty()
        || bytes.len() > fe2o3_verifier::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3
        || <[u8; 32]>::from(Sha256::digest(bytes)) != observation.model
    {
        return Err(invalid(
            "model export differs from the observed bounded model",
        ));
    }
    let request_bytes = bounded_input(request)?;
    let Arguments(args) = serde_json::from_slice(&request_bytes)?;
    let rust_source = bounded_input(source)?;
    let target = flag(&args, "-Ctarget-cpu=")?;
    if !matches!(target, "gfx942" | "gfx950") {
        return Err(invalid("unexpected actual-model test target"));
    }
    let opt = flag(&args, "-Copt-level=")?
        .parse::<u8>()
        .map_err(|_| invalid("invalid codegen optimization level"))?;
    let mir = flag(&args, "-Zmir-opt-level=")?
        .parse::<u8>()
        .map_err(|_| invalid("invalid MIR optimization level"))?;
    let stem = file_stem(response)?;
    let record = Record {
        format: "fe2o3-actual-expanded-model-export-v282",
        qualification: false,
        target,
        codegen_opt_level: opt,
        mir_opt_level: mir,
        case: file_stem(source)?,
        request_sha256: Sha256::digest(&request_bytes).into(),
        rust_source_sha256: Sha256::digest(&rust_source).into(),
        request_file: format!("{stem}.args.json"),
        source_file: format!("{stem}.source.rs"),
        model_file: format!("{stem}.rs"),
        model_bytes: bytes.len(),
        observation,
    };
    let record_bytes = serde_json::to_vec(&record)?;
    if record_bytes.len() > RECORD_LIMIT {
        return Err(invalid("model export record exceeds diagnostic limit"));
    }
    create_new(&directory.join(&record.model_file), bytes)?;
    create_new(&directory.join(&record.request_file), &request_bytes)?;
    create_new(&directory.join(&record.source_file), &rust_source)?;
    create_new(&directory.join(format!("{stem}.json")), &record_bytes)
}

pub(super) fn observe(
    bytes: &[u8],
    observation: &ModelObservation,
    budget: &mut Budget<'_>,
) -> Result<(), SourceError> {
    let Some(directory) = env::var_os(EXPANDED_MODEL_EXPORT_V282) else {
        return Ok(());
    };
    let floor = budget.storage();
    budget.with_prepaid_scope(floor, 1, 1, 4096, |budget| {
        // Input buffers, decoded bounded JSON and record serialization are all
        // diagnostic scratch. The generated model itself is borrowed in place.
        budget.reserve_storage(8 * (INPUT_LIMIT + 1) + 2 * RECORD_LIMIT)?;
        budget.charge_work(bytes.len() + 8 * INPUT_LIMIT + 2 * RECORD_LIMIT)?;
        let result = (|| {
            let request = env::var_os(ARGS).ok_or_else(|| invalid("missing model request"))?;
            let response = env::var_os(RESULT).ok_or_else(|| invalid("missing model response"))?;
            let source = env::var_os("FE2O3_CONTEXT_PROTOCOL_SOURCE")
                .ok_or_else(|| invalid("missing actual Rust source"))?;
            export(
                Path::new(&directory),
                Path::new(&request),
                Path::new(&response),
                Path::new(&source),
                bytes,
                observation,
            )
        })();
        result.map_err(|error| {
            eprintln!("actual expanded model export: {error}");
            SourceError::Unsupported("actual expanded model diagnostic export failed")
        })
    })
}

#[test]
fn expanded_model_export_names_and_compiler_flags_are_bounded_and_exact() {
    assert_eq!(
        file_stem(Path::new("response-gfx942-3-0-4.json")).unwrap(),
        "response-gfx942-3-0-4"
    );
    for bad in [".json", "bad_name.json", "bad name.json"] {
        assert!(file_stem(Path::new(bad)).is_err());
    }
    let args = vec!["-Copt-level=3".to_owned()];
    assert_eq!(flag(&args, "-Copt-level=").unwrap(), "3");
    assert!(flag(&args, "-Zmir-opt-level=").is_err());
    assert!(flag(&[args[0].clone(), args[0].clone()], "-Copt-level=").is_err());
}

#[test]
fn expanded_model_export_never_overwrites_an_existing_file() {
    let temporary = crate::test_temp_dir::TestTempDir::create("fe2o3-model-export-v282");
    let path = temporary.path().join("model.rs");
    create_new(&path, b"retained").unwrap();
    assert_eq!(
        create_new(&path, b"replacement").unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(std::fs::read(path).unwrap(), b"retained");
}

#[test]
fn expanded_model_export_argument_decoder_limits_allocation_before_growth() {
    let exact = serde_json::to_vec(&vec![""; 128]).unwrap();
    assert_eq!(
        serde_json::from_slice::<Arguments>(&exact).unwrap().0.len(),
        128
    );
    let excess = serde_json::to_vec(&vec![""; 129]).unwrap();
    assert!(serde_json::from_slice::<Arguments>(&excess).is_err());
    assert!(serde_json::from_slice::<Arguments>(br#"["rustc",42]"#).is_err());
}

#[test]
fn expanded_model_export_preserves_observed_bytes_and_original_input_digests() {
    let temporary = crate::test_temp_dir::TestTempDir::create("fe2o3-model-export-v282");
    let root = temporary.path().canonicalize().unwrap();
    let directory = root.join("exports");
    std::fs::create_dir(&directory).unwrap();
    let request = root.join("request.json");
    let response = root.join("response-gfx942-3-0-4.json");
    let source = root.join("helper.rs");
    let args = br#"["rustc","-Ctarget-cpu=gfx942","-Copt-level=3","-Zmir-opt-level=0"]"#;
    create_new(&request, args).unwrap();
    create_new(&source, b"fn example() {}\n").unwrap();
    // Synthetic bytes exercise file custody only, not Verus or compiler admission.
    let bytes = b"synthetic export fixture";
    let observation = ModelObservation {
        graphs: [[0; 32]; 3],
        runtime_and_instances: [0; 32],
        references: [0; 32],
        model: Sha256::digest(bytes).into(),
        census: [0; 32],
        counts: [0; 6],
        helper_instances: [0; 2],
        model_consumer_called: true,
    };
    export(
        &directory,
        &request,
        &response,
        &source,
        bytes,
        &observation,
    )
    .unwrap();
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 4);
    let record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(directory.join(response.file_name().unwrap())).unwrap(),
    )
    .unwrap();
    assert_eq!(record["qualification"], false);
    assert_eq!(record["target"], "gfx942");
    assert_eq!(record["codegen_opt_level"], 3);
    assert_eq!(record["mir_opt_level"], 0);
    assert_eq!(record["case"], "helper");
    let recorded: ModelObservation = serde_json::from_value(record["observation"].clone()).unwrap();
    assert_eq!(recorded, observation);
    for (key, expected) in [
        ("model_file", bytes.as_slice()),
        ("request_file", args.as_slice()),
        ("source_file", b"fn example() {}\n".as_slice()),
    ] {
        assert_eq!(
            std::fs::read(directory.join(record[key].as_str().unwrap())).unwrap(),
            expected
        );
    }
    assert_eq!(
        record["request_sha256"],
        serde_json::to_value(<[u8; 32]>::from(Sha256::digest(args))).unwrap()
    );
    assert_eq!(
        record["rust_source_sha256"],
        serde_json::to_value(<[u8; 32]>::from(Sha256::digest(b"fn example() {}\n"))).unwrap()
    );
    assert_eq!(
        export(
            &directory,
            &request,
            &response,
            &source,
            bytes,
            &observation
        )
        .unwrap_err()
        .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 4);
}

#[test]
fn expanded_model_export_refuses_unbounded_inputs_and_mismatched_observations() {
    let temporary = crate::test_temp_dir::TestTempDir::create("fe2o3-model-export-v282");
    let root = temporary.path().canonicalize().unwrap();
    let input = root.join("oversized.json");
    let file = std::fs::File::create(&input).unwrap();
    file.set_len(INPUT_LIMIT as u64 + 1).unwrap();
    drop(file);
    assert_eq!(
        bounded_input(&input).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    let observation = ModelObservation {
        graphs: [[0; 32]; 3],
        runtime_and_instances: [0; 32],
        references: [0; 32],
        model: [0; 32],
        census: [0; 32],
        counts: [0; 6],
        helper_instances: [0; 2],
        model_consumer_called: true,
    };
    for bytes in [b"".as_slice(), b"wrong model".as_slice()] {
        assert_eq!(
            export(&root, &input, &input, &input, bytes, &observation)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
}

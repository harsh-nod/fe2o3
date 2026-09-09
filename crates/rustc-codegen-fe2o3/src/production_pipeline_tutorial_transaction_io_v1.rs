fn publish_transaction_export(
    destination: &Path,
    result_name: &str,
    envelope: &[u8],
    objects: &BTreeMap<String, Vec<u8>>,
    hardware_archive: Option<&[u8]>,
) -> ResultV1<()> {
    if hardware_archive
        .is_some_and(|archive| archive.is_empty() || archive.len() > MAX_EVIDENCE_BYTES)
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::MissingEvidence,
            "hardware archive is empty or oversized",
        );
    }
    let parent = destination.parent().ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::OutputPath,
            "output directory has no parent",
        )
    })?;
    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let stage = parent.join(format!(
        ".fe2o3-tutorial-transaction-{}-{sequence}",
        std::process::id()
    ));
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder.create(&stage).map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::Publication,
            format!("cannot create transaction staging directory: {error}"),
        )
    })?;
    let outcome = (|| {
        write_new_file(&stage.join(result_name), envelope)?;
        if let Some(archive) = hardware_archive {
            write_new_file(&stage.join("hardware-archive-v1.zip"), archive)?;
        }
        let mut written = BTreeMap::<String, &[u8]>::new();
        for payload in objects.values() {
            let digest = hex_sha256(payload);
            if let Some(existing) = written.get(&digest) {
                if existing != payload {
                    return fail(
                        TutorialProductionTransactionErrorCodeV1::Publication,
                        "content-addressed evidence collision",
                    );
                }
                continue;
            }
            let path = stage.join(OBJECT_PREFIX).join(&digest[..2]).join(&digest);
            if let Some(parent) = path.parent() {
                let mut builder = fs::DirBuilder::new();
                builder.recursive(true).mode(0o700);
                builder.create(parent).map_err(|error| {
                    TutorialProductionTransactionErrorV1::new(
                        TutorialProductionTransactionErrorCodeV1::Publication,
                        format!("cannot create evidence object directory: {error}"),
                    )
                })?;
            }
            write_new_file(&path, payload)?;
            written.insert(digest, payload);
        }
        sync_directories(&stage)?;
        rename_noreplace(&stage, destination).map_err(|error| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::Publication,
                format!("cannot atomically publish transaction export: {error}"),
            )
        })?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                TutorialProductionTransactionErrorV1::new(
                    TutorialProductionTransactionErrorCodeV1::Publication,
                    format!("cannot sync transaction publication parent: {error}"),
                )
            })
    })();
    if outcome.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    outcome
}

fn sync_directories(root: &Path) -> ResultV1<()> {
    let mut directories = vec![root.to_path_buf()];
    let mut cursor = 0;
    while cursor < directories.len() {
        let directory = directories[cursor].clone();
        cursor += 1;
        for entry in fs::read_dir(&directory)
            .map_err(|error| io_error("cannot enumerate transaction staging directory", error))?
        {
            let entry = entry
                .map_err(|error| io_error("cannot inspect transaction staging entry", error))?;
            if entry
                .file_type()
                .map_err(|error| io_error("cannot inspect transaction staging type", error))?
                .is_dir()
            {
                directories.push(entry.path());
            }
        }
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        File::open(&directory)
            .and_then(|file| file.sync_all())
            .map_err(|error| {
                TutorialProductionTransactionErrorV1::new(
                    TutorialProductionTransactionErrorCodeV1::Publication,
                    format!("cannot sync {}: {error}", directory.display()),
                )
            })?;
    }
    Ok(())
}

fn rename_noreplace(source: &Path, destination: &Path) -> std::io::Result<()> {
    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: both C strings are NUL-terminated, valid for the call, and no file descriptors are
    // borrowed. `RENAME_NOREPLACE` preserves the producer's new-path publication contract.
    let status = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn write_new_file(path: &Path, bytes: &[u8]) -> ResultV1<()> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    let mut file = options.open(path).map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::Publication,
            format!("cannot create {}: {error}", path.display()),
        )
    })?;
    file.write_all(bytes).map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::Publication,
            format!("cannot write {}: {error}", path.display()),
        )
    })?;
    file.sync_all().map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::Publication,
            format!("cannot sync {}: {error}", path.display()),
        )
    })
}

fn require_new_output_path(path: &Path) -> ResultV1<()> {
    if path.exists() || fs::symlink_metadata(path).is_ok() {
        return fail(
            TutorialProductionTransactionErrorCodeV1::OutputPath,
            "transaction output directory must be a new path",
        );
    }
    let parent = path.parent().ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::OutputPath,
            "transaction output directory has no parent",
        )
    })?;
    let resolved = real_directory(parent, "transaction output parent")?;
    if resolved != parent {
        return fail(
            TutorialProductionTransactionErrorCodeV1::OutputPath,
            "transaction output parent must be an exact non-symlink path",
        );
    }
    Ok(())
}

fn require_external_output_path(repository: &Path, path: &Path) -> ResultV1<()> {
    require_new_output_path(path)?;
    let parent = path.parent().expect("new output path has a checked parent");
    let parent = real_directory(parent, "transaction output parent")?;
    if parent.starts_with(repository) {
        return fail(
            TutorialProductionTransactionErrorCodeV1::OutputPath,
            "transaction output must be outside the compiler worktree",
        );
    }
    Ok(())
}

fn read_operator_policy(repository: &Path, path: &Path) -> ResultV1<Vec<u8>> {
    if !path.is_absolute() {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestIo,
            "hardware trust policy path must be absolute",
        );
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| io_error("cannot resolve hardware trust policy", error))?;
    if canonical != path || canonical.starts_with(repository) {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestIo,
            "hardware trust policy must be an exact operator-owned path outside the worktree",
        );
    }
    let bytes = read_regular(path, MAX_JSON_BYTES, "hardware trust policy")?;
    parse_canonical_document(&bytes, "hardware trust policy")?;
    Ok(bytes)
}

fn validate_reference(value: &Value, label: &str) -> ResultV1<()> {
    let reference = object(value, label)?;
    exact_keys(reference, &["bytes", "path", "sha256"], label)?;
    let digest = sha_field(value, "sha256", label)?;
    let bytes = u64_field(value, "bytes", label)?;
    if bytes == 0 || bytes > MAX_EVIDENCE_BYTES as u64 {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label}.bytes is outside the evidence bound"),
        );
    }
    if string_field(value, "path", label)? != format!("{OBJECT_PREFIX}/{}/{digest}", &digest[..2]) {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label}.path is not canonical content-addressed storage"),
        );
    }
    Ok(())
}

fn object_reference(payload: &[u8]) -> Value {
    let digest = hex_sha256(payload);
    serde_json::json!({
        "bytes": payload.len(),
        "path": format!("{OBJECT_PREFIX}/{}/{digest}", &digest[..2]),
        "sha256": digest,
    })
}

fn require_exact_evidence(
    objects: &BTreeMap<String, Vec<u8>>,
    kind: &str,
    expected: &[u8],
) -> ResultV1<()> {
    if evidence(objects, kind)? != expected {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            format!("{kind} evidence differs from compiler-owned canonical bytes"),
        );
    }
    Ok(())
}

fn require_hash_evidence(
    objects: &BTreeMap<String, Vec<u8>>,
    kind: &str,
    expected: [u8; 32],
) -> ResultV1<()> {
    if sha256(evidence(objects, kind)?) != expected {
        return fail(
            TutorialProductionTransactionErrorCodeV1::EvidenceMismatch,
            format!("{kind} evidence differs from compiler-owned identity"),
        );
    }
    Ok(())
}

fn require_exact_measurement(
    expected_sha256: [u8; 32],
    expected_bytes: u64,
    actual: &[u8],
    label: &str,
) -> ResultV1<()> {
    if sha256(actual) != expected_sha256 || actual.len() as u64 != expected_bytes {
        return fail(
            TutorialProductionTransactionErrorCodeV1::ArtifactMismatch,
            format!("{label} differs from the completed compiler-stage receipt"),
        );
    }
    Ok(())
}

fn evidence<'a>(objects: &'a BTreeMap<String, Vec<u8>>, kind: &str) -> ResultV1<&'a [u8]> {
    objects.get(kind).map(Vec::as_slice).ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::MissingEvidence,
            format!("production transaction omits {kind} evidence"),
        )
    })
}

fn read_repository_file(repository: &Path, relative: &str, maximum: u64) -> ResultV1<Vec<u8>> {
    let path = protected_relative(repository, relative, "repository input")?;
    read_regular(&path, maximum, "repository input")
}

fn protected_relative(root: &Path, relative: &str, label: &str) -> ResultV1<PathBuf> {
    if relative.is_empty() || !relative.is_ascii() || relative.contains('\\') {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            format!("{label} is not a portable repository-relative path"),
        );
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            format!("{label} escapes the repository"),
        );
    }
    let joined = root.join(path);
    let metadata = fs::symlink_metadata(&joined)
        .map_err(|error| io_error(&format!("cannot inspect {label}"), error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            format!("{label} must be a regular non-symlink file"),
        );
    }
    let canonical = joined
        .canonicalize()
        .map_err(|error| io_error(&format!("cannot resolve {label}"), error))?;
    if !canonical.starts_with(root) || canonical != joined {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            format!("{label} traverses a symlink or escapes the repository"),
        );
    }
    Ok(joined)
}

fn read_regular(path: &Path, maximum: u64, label: &str) -> ResultV1<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error(&format!("cannot inspect {label}"), error))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > maximum
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestIo,
            format!("{label} must be a bounded, nonempty regular non-symlink file"),
        );
    }
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    let mut file = options
        .open(path)
        .map_err(|error| io_error(&format!("cannot open {label}"), error))?;
    let before = file
        .metadata()
        .map_err(|error| io_error(&format!("cannot inspect open {label}"), error))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| io_error(&format!("cannot read {label}"), error))?;
    let after = file
        .metadata()
        .map_err(|error| io_error(&format!("cannot reinspect {label}"), error))?;
    if bytes.len() as u64 != metadata.len() || !same_file(&before, &after) {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestIo,
            format!("{label} changed while it was read"),
        );
    }
    Ok(bytes)
}

fn real_directory(path: &Path, label: &str) -> ResultV1<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error(&format!("cannot inspect {label}"), error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return fail(
            TutorialProductionTransactionErrorCodeV1::OutputPath,
            format!("{label} must be a real directory"),
        );
    }
    path.canonicalize()
        .map_err(|error| io_error(&format!("cannot resolve {label}"), error))
}

fn same_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    (
        before.dev(),
        before.ino(),
        before.len(),
        before.mtime(),
        before.mtime_nsec(),
        before.ctime(),
        before.ctime_nsec(),
    ) == (
        after.dev(),
        after.ino(),
        after.len(),
        after.mtime(),
        after.mtime_nsec(),
        after.ctime(),
        after.ctime_nsec(),
    )
}

fn git(repository: &Path, arguments: &[&str]) -> ResultV1<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| io_error("cannot execute git", error))?;
    if !output.status.success() {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            format!(
                "git {} failed: {}",
                arguments.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        );
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::SourceMismatch,
                "git emitted non-UTF-8 output",
            )
        })
}

fn parse_canonical_document(bytes: &[u8], label: &str) -> ResultV1<Value> {
    let Some(json) = bytes.strip_suffix(b"\n") else {
        return fail(
            TutorialProductionTransactionErrorCodeV1::NonCanonicalJson,
            format!("{label} must end in exactly one newline"),
        );
    };
    let value: Value = serde_json::from_slice(json).map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::NonCanonicalJson,
            format!("cannot decode {label}: {error}"),
        )
    })?;
    if canonical_json(&value)? != json {
        return fail(
            TutorialProductionTransactionErrorCodeV1::NonCanonicalJson,
            format!("{label} is not canonical JSON followed by one newline"),
        );
    }
    Ok(value)
}

/// Parses a repository-owned JSON document while keeping its exact raw bytes as the identity.
///
/// Repository manifests are intentionally reviewable, pretty-printed JSON. The caller binds the
/// exact file bytes separately; this parser enforces the same bounded value domain as canonical
/// transaction JSON and rejects duplicate object keys without requiring a compact presentation.
fn parse_repository_document(bytes: &[u8], label: &str) -> ResultV1<Value> {
    if bytes.is_empty() || !bytes.ends_with(b"\n") {
        return fail(
            TutorialProductionTransactionErrorCodeV1::NonCanonicalJson,
            format!("{label} must be a nonempty JSON document ending in a newline"),
        );
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let UniqueValue(value) = UniqueValue::deserialize(&mut deserializer).map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::NonCanonicalJson,
            format!("cannot decode {label}: {error}"),
        )
    })?;
    deserializer.end().map_err(|error| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::NonCanonicalJson,
            format!("cannot decode {label}: {error}"),
        )
    })?;
    canonical_json(&value)?;
    Ok(value)
}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = UniqueValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueValue)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_string(value.to_owned())
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueValue>()? {
            values.push(value.0);
        }
        Ok(UniqueValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some((key, value)) = object.next_entry::<String, UniqueValue>()? {
            if values.insert(key.clone(), value.0).is_some() {
                return Err(de::Error::custom(format!("duplicate JSON key {key:?}")));
            }
        }
        Ok(UniqueValue(Value::Object(values)))
    }
}

fn canonical_document(value: &Value) -> ResultV1<Vec<u8>> {
    let mut bytes = canonical_json(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn canonical_json(value: &Value) -> ResultV1<Vec<u8>> {
    let mut output = Vec::new();
    encode_canonical(value, &mut output)?;
    Ok(output)
}

fn encode_canonical(value: &Value, output: &mut Vec<u8>) -> ResultV1<()> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::Number(number) if number.is_i64() || number.is_u64() => {
            output.extend_from_slice(number.to_string().as_bytes());
        }
        Value::Number(_) => {
            return fail(
                TutorialProductionTransactionErrorCodeV1::NonCanonicalJson,
                "floating-point JSON numbers are forbidden in transaction evidence",
            );
        }
        Value::String(string) => {
            if !string.is_ascii() {
                return fail(
                    TutorialProductionTransactionErrorCodeV1::NonCanonicalJson,
                    "transaction JSON strings must use the ASCII contract domain",
                );
            }
            let encoded = serde_json::to_string(string).map_err(|error| {
                TutorialProductionTransactionErrorV1::new(
                    TutorialProductionTransactionErrorCodeV1::NonCanonicalJson,
                    format!("cannot encode canonical string: {error}"),
                )
            })?;
            output.extend_from_slice(encoded.as_bytes());
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                encode_canonical(value, output)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            for (index, (key, value)) in entries.into_iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                encode_canonical(&Value::String(key.clone()), output)?;
                output.push(b':');
                encode_canonical(value, output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str], label: &str) -> ResultV1<()> {
    let observed = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if observed != expected {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label} fields differ"),
        );
    }
    Ok(())
}

fn object<'a>(value: &'a Value, label: &str) -> ResultV1<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label} must be an object"),
        )
    })
}

fn required<'a>(object: &'a Map<String, Value>, field: &str, label: &str) -> ResultV1<&'a Value> {
    object.get(field).ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label} omits {field}"),
        )
    })
}

fn string_field<'a>(value: &'a Value, field: &str, label: &str) -> ResultV1<&'a str> {
    required(object(value, label)?, field, label)?
        .as_str()
        .ok_or_else(|| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::RequestSchema,
                format!("{label}.{field} must be a string"),
            )
        })
}

fn identity_field<'a>(value: &'a Value, field: &str, label: &str) -> ResultV1<&'a str> {
    let identity = string_field(value, field, label)?;
    if identity.is_empty()
        || !identity.is_ascii()
        || !identity
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label}.{field} is not a portable identity"),
        );
    }
    Ok(identity)
}

fn sha_field<'a>(value: &'a Value, field: &str, label: &str) -> ResultV1<&'a str> {
    let digest = string_field(value, field, label)?;
    if digest.len() != 64
        || digest == "0000000000000000000000000000000000000000000000000000000000000000"
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label}.{field} is not a nonzero lowercase SHA-256 identity"),
        );
    }
    Ok(digest)
}

fn u64_field(value: &Value, field: &str, label: &str) -> ResultV1<u64> {
    required(object(value, label)?, field, label)?
        .as_u64()
        .ok_or_else(|| {
            TutorialProductionTransactionErrorV1::new(
                TutorialProductionTransactionErrorCodeV1::RequestSchema,
                format!("{label}.{field} must be an unsigned integer"),
            )
        })
}

fn sorted_unique_strings(value: &Value, label: &str) -> ResultV1<Vec<String>> {
    let values = value.as_array().ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label} must be an array"),
        )
    })?;
    let strings = values
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                TutorialProductionTransactionErrorV1::new(
                    TutorialProductionTransactionErrorCodeV1::RequestSchema,
                    format!("{label} must contain only strings"),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut sorted = strings.clone();
    sorted.sort();
    sorted.dedup();
    if strings.is_empty() || strings != sorted {
        return fail(
            TutorialProductionTransactionErrorCodeV1::RequestSchema,
            format!("{label} must be sorted, unique, and nonempty"),
        );
    }
    Ok(strings)
}

fn portable_path(path: &Path, label: &str) -> ResultV1<String> {
    let path = path.to_str().ok_or_else(|| {
        TutorialProductionTransactionErrorV1::new(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            format!("{label} path is not UTF-8"),
        )
    })?;
    if !path.is_ascii() || path.contains('\\') {
        return fail(
            TutorialProductionTransactionErrorCodeV1::SourceMismatch,
            format!("{label} path is not portable"),
        );
    }
    Ok(path.to_owned())
}

fn domain_sha256(domain: &[u8], value: &Value) -> ResultV1<String> {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(canonical_json(value)?);
    Ok(hex32(digest.finalize().into()))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex_sha256(bytes: &[u8]) -> String {
    hex32(sha256(bytes))
}

fn hex32(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn io_error(context: &str, error: std::io::Error) -> TutorialProductionTransactionErrorV1 {
    TutorialProductionTransactionErrorV1::new(
        TutorialProductionTransactionErrorCodeV1::RequestIo,
        format!("{context}: {error}"),
    )
}

fn fail<T>(
    code: TutorialProductionTransactionErrorCodeV1,
    message: impl Into<String>,
) -> ResultV1<T> {
    Err(TutorialProductionTransactionErrorV1::new(code, message))
}

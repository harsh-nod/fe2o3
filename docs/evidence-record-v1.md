# Auditable Evidence Result Record V1

`scripts/parity-evidence.sh` has two deliberately separate evidence layers:

- `collect` and `validate` preserve the existing canonical 118-line parity
  declaration. A declaration identifies a checkout, a toolchain, a hardware
  lane, and 109 row links. It does not prove that a command ran or passed.
- `record` and `verify-record` produce and verify an auditable command-result
  record. A passing V1 record binds one exact invocation to one detached Git
  commit, its scrubbed environment, executable files, durable combined output,
  and declared output artifacts.

No existing declaration is restamped or promoted by introducing result-record
V1. Status changes still require the parity matrix Definition of Done and the
dashboard claim gate.

## Recording a Result

Use a clean detached checkout. The archive must be outside that checkout so
creating evidence cannot dirty the source tree.

```bash
git switch --detach <commit>
test -z "$(git status --porcelain=v1 --untracked-files=all)"

mkdir -p /evidence/run-001
scripts/parity-evidence.sh record \
  --repo "$PWD" \
  --archive-root /evidence/run-001 \
  --record records/host-tests.tsv \
  --log logs/host-tests.log \
  --env PATH \
  --env HOME \
  --env CARGO_TARGET_DIR=/tmp/fe2o3-host-target \
  --tool cargo=/absolute/path/to/cargo \
  --tool rustc=/absolute/path/to/rustc \
  --artifact test-binary=artifacts/fe2o3-host-tests \
  -- /absolute/path/to/cargo test -p fe2o3-host --locked
```

The command executable must be an absolute path. The command runs from the
repository root under `env -i`; only `LC_ALL=C` and variables supplied with
`--env` are present. `--env NAME` captures the current value, while
`--env NAME=VALUE` supplies an exact value. Do not record secrets.

The archive, record, and log must already have suitable durable storage. The
script flushes the log and record with `sync -f` when supported, but it cannot
provide storage durability beyond the filesystem and archive service.

If the command exits nonzero, the script retains a record and returns the same
status. Such a record is diagnostic evidence; `verify-record` rejects it as a
passing result. Missing declared artifacts and source-tree changes fail record
creation.

## Running a Bounded Snapshot

`scripts/run-parity-snapshot.sh` builds independent result records on top of
`record` and `verify-record`. It has a static V1 plan rather than accepting
arbitrary commands:

| Shard | Bounded coverage |
|---|---|
| Q1 | Workspace tests plus parity matrix, dashboard, and evidence validators |
| Q2 | `rustc-codegen-fe2o3` tests |
| Q3 | MIR, Kernel IR, kernel analysis, and AMDGPU lowering tests |
| Q4 | Artifact, transaction, HSACO inspection, and finalization tests |
| Q5 | Core, HIP, device, completion, verifier, host, and HSA runtime tests |
| Q6 | Cargo integration, differential unit tests, and conformance harness |
| Q7 | Positive and negative Verus fixtures |
| GFX942-COMPILE | Optional gfx942 ROCm compilation and two-kernel publication |
| GFX942-HARDWARE | Optional gfx942 identity and generated vecadd execution |

List the plan without inspecting a checkout:

```bash
scripts/run-parity-snapshot.sh list
```

Every other mode requires an existing archive outside a clean detached
checkout. With no `--shard`, the seven Q shards are selected in order. Q7 also
requires an exact Verus executable. Optional gfx942 lanes are never selected
implicitly.

```bash
mkdir -p /evidence/snapshot-001
git switch --detach <commit>

scripts/run-parity-snapshot.sh dry-run \
  --repo "$PWD" \
  --archive-root /evidence/snapshot-001 \
  --verus /absolute/path/to/verus

scripts/run-parity-snapshot.sh run \
  --repo "$PWD" \
  --archive-root /evidence/snapshot-001 \
  --verus /absolute/path/to/verus
```

`dry-run` emits a deterministic canonical plan containing the commit, archive
paths, hex-encoded environment and outer command argv, and exact tool paths. It
does not create records or work directories. Machine-specific values such as
`PATH`, `HOME`, `CARGO_HOME`, and `RUSTUP_HOME` can be supplied explicitly;
every `PATH` entry must be absolute.

`run` preflights every selected record, log, and work path before starting the
first shard. Each shard gets a fresh `work/<shard>/target`, `tmp`, `output`, and
CI log directory. The command runs with the recorder's empty-environment
policy, a per-shard timeout, and the exact environment shown by `dry-run`.
Commands within a shard stop at the first failure. A failed record is retained,
the runner returns that status, and no later shard starts. Each successful
record is immediately passed to `verify-record` before the next shard runs.

Use repeated `--shard` options to run or verify a subset:

```bash
scripts/run-parity-snapshot.sh run \
  --repo "$PWD" \
  --archive-root /evidence/snapshot-001 \
  --shard Q3 --shard Q4

scripts/run-parity-snapshot.sh verify-only \
  --repo "$PWD" \
  --archive-root /evidence/snapshot-001 \
  --shard Q3 --shard Q4
```

The gfx942 hardware lane requires the exact vecadd HSACO to be a regular,
non-symlink file inside the archive. Its size and digest are bound as a record
artifact, while its absolute path is recorded in the command environment:

```bash
scripts/run-parity-snapshot.sh run \
  --repo "$PWD" \
  --archive-root /evidence/snapshot-001 \
  --shard Q5 \
  --gfx942-hardware \
  --vecadd-hsaco artifacts/gfx942-vecadd.hsaco
```

Snapshot orchestration creates result evidence only. It does not update a row
link, restamp a declaration, promote a parity status, regenerate the matrix or
dashboard, or turn a compile observation into a hardware-execution claim.

## Canonical Schema

A record is bounded to 1 MiB and uses canonical TSV in this order:

1. `record_schema_version`, exact Git commit, and true detached/clean-before/
   clean-after assertions.
2. Counted, indexed argv values encoded as lowercase byte hex. `-` represents
   an empty value.
3. Counted environment entries, sorted by name, with hex-encoded values.
4. Counted tool entries, sorted by label, containing an absolute path and the
   executable file's SHA-256.
5. Exit status plus archive-relative log path, byte size, and SHA-256.
6. Counted artifacts, sorted by label, each with an archive-relative path,
   byte size, and SHA-256.
7. `record_sha256`, the SHA-256 of every preceding canonical record byte.

Paths are bounded, traversal-free, and confined to the resolved archive root.
Logs, artifacts, and the record must be regular non-symlink files. Verification
checks the current executable digests and requires the verification checkout to
be clean, detached, and at the recorded commit.

```bash
scripts/parity-evidence.sh verify-record \
  --repo /checkout/at-recorded-commit \
  --archive-root /evidence/run-001 \
  records/host-tests.tsv
```

The final record checksum detects accidental or partial modification. It is
not a signature: an attacker able to rewrite the archive can rewrite content
and recompute hashes. Authentic provenance requires a separately access-
controlled or signed archive manifest.

## Migration From Declarations

Existing declarations remain schema version 1 and continue to validate
byte-for-byte. Migrate evidence without changing status claims in this order:

1. Rerun each evidence lane from a clean detached checkout at the intended
   snapshot commit, using a distinct archive and build directory.
2. Retain each result record, log, and declared artifact together.
3. Run `verify-record` against the archived result and recorded checkout.
4. Review the command's semantic coverage against the row's Definition of Done.
5. Update the declaration row link to the reviewed result only after that
   review. Use an archive-relative link such as
   `records/host-tests.tsv#sha256-<record_sha256>`.
6. Regenerate the dashboard through its normal claim gate. A passing command
   alone does not justify a row promotion.

The legacy declaration validator checks row-link syntax, not archive contents.
The result record and its files must therefore be verified explicitly before a
row link is accepted in review. This is the V1 migration boundary; automatic
declaration-to-archive resolution is a later schema capability.

## V1 Limits

- One record represents one process invocation and its combined stdout/stderr.
- V1 does not sign records, attest the machine, or prove that a named hardware
  device executed the workload.
- Tool identity is executable-file identity, not the closure of dynamic
  libraries, firmware, containers, or operating-system state.
- Artifacts must be explicitly declared. Undeclared outputs have no evidentiary
  authority.
- Concurrent, distributed, and hardware lanes need separate result records.

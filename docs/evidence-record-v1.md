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

The optional gfx942 compile shard requires all four Worker V2 identities. The
runner validates them, includes them in the canonical dry-run plan and result
record, and passes only those exact values through its empty environment:

```bash
scripts/run-parity-snapshot.sh run \
  --repo "$PWD" \
  --archive-root /evidence/snapshot-001 \
  --gfx942-compile \
  --llvm-link-worker /absolute/path/to/fe2o3-llvm-link-worker \
  --llvm-link-worker-build-id fe2o3-worker-v1-sha256-<digest> \
  --llvm-build-id rocm-7.2.4 \
  --llvm-as /absolute/path/to/llvm-as
```

Ambient `FE2O3_LLVM_LINK_WORKER`, `FE2O3_LLVM_LINK_WORKER_BUILD_ID`,
`FE2O3_LLVM_BUILD_ID`, and `FE2O3_LLVM_AS` values are ignored. Missing,
malformed, non-executable, or inconsistent options fail before any shard runs.
The compile shard records compilation and Worker V2 publication only; it is not
hardware execution evidence.

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

The raw alpha/zeta vertical slice landed through
`daf0b459ced07a25376670c83b1474eaebcd1a68` is not a shard in this static
snapshot plan. The generated-safe fake-authenticator execution added at
`dc9738e367c392f7716eacb8459ca73fa32abbbb` has a separate optional shard.
The following commands reproduce the measured Worker V2 build and export on a
ROCm 7.2.4 system. The worker measurement is derived by CMake
from its pinned LLVM/LLD configuration and worker sources, not from the output
binary alone:

```bash
cmake -S tools/fe2o3-llvm-link-worker -B /absolute/path/to/worker-build \
  -DLLVM_DIR=/opt/rocm-7.2.4/lib/llvm/lib/cmake/llvm \
  -DLLD_DIR=/opt/rocm-7.2.4/lib/llvm/lib/cmake/lld \
  -DFE2O3_PINNED_LLVM_VERSION=22.0.0git \
  -DFE2O3_LLVM_BUILD_ID_FILE=/opt/rocm/.info/version \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID=7.2.4 \
  -DBUILD_TESTING=ON -DCMAKE_BUILD_TYPE=Release
cmake --build /absolute/path/to/worker-build --parallel
ctest --test-dir /absolute/path/to/worker-build --output-on-failure
cat /absolute/path/to/worker-build/fe2o3-worker-build-id.txt

test ! -e /absolute/path/to/alpha-zeta-cov6.hsaco
FE2O3_LLVM_LINK_WORKER=/absolute/path/to/worker-build/fe2o3-llvm-link-worker \
FE2O3_LLVM_LINK_WORKER_BUILD_ID=fe2o3-worker-v1-sha256-234d22f9fb347c86495e7156e53ef8eab55e939d6514973a6df373aee12f77a9 \
FE2O3_LLVM_BUILD_ID=7.2.4 \
FE2O3_GFX942_ALPHA_ZETA_OUTPUT=/absolute/path/to/alpha-zeta-cov6.hsaco \
cargo +nightly-2026-04-03 test --locked -p rustc-codegen-fe2o3 \
  --test kernel_ir_codegen \
  worker_v2_general_v3_alpha_zeta_build_links_and_validate_backend_witnesses \
  -- --ignored --exact --nocapture
sha256sum /absolute/path/to/alpha-zeta-cov6.hsaco
/opt/rocm/llvm/bin/llvm-readelf --notes \
  /absolute/path/to/alpha-zeta-cov6.hsaco
```

The observed worker identity was
`fe2o3-worker-v1-sha256-234d22f9fb347c86495e7156e53ef8eab55e939d6514973a6df373aee12f77a9`.
The exported COV6 HSACO SHA-256 was
`3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4`.
Its AMDHSA metadata reported complete kernarg sizes of `296` bytes for `alpha`
and `312` bytes for `zeta`, including each 256-byte COV6 implicit suffix. This
confirms that the earlier explicit-versus-complete kernarg mismatch is fixed.

The same digest-pinned artifact was then executed on an AMD Instinct MI300X,
`gfx942:xnack-`, with ROCm 7.2.4:

```bash
FE2O3_RUN_GFX942_TWO_KERNEL=1 \
FE2O3_GFX942_ALPHA_ZETA_HSACO=/absolute/path/to/alpha-zeta-cov6.hsaco \
FE2O3_GFX942_ALPHA_ZETA_SHA256=3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4 \
cargo +nightly-2026-04-03 test --locked -p fe2o3-hsa-runtime \
  --features hardware-test-hooks --test gfx942_two_kernel_hardware \
  gfx942_cov6_alpha_then_zeta_one_executable \
  -- --ignored --exact --nocapture
```

That run passed independent CPU-oracle and canary checks at lengths `1`, `255`,
`256`, `257`, and `1023`. It calls the reviewed raw unsafe packing path, not the
landed generated alpha/zeta safe dispatch SPI, and it was not captured by the
clean detached-checkout V1 snapshot runner. It is therefore an observed raw
hardware result, not a dashboard `remote-hardware` strength or a parity row
promotion.

At commit `dc9738e367c392f7716eacb8459ca73fa32abbbb`, the same host, toolchain,
artifact digest, and length matrix also passed through the generated-safe SPI:

```bash
FE2O3_RUN_GFX942_TWO_KERNEL=1 \
FE2O3_GFX942_ALPHA_ZETA_HSACO=/absolute/path/to/alpha-zeta-cov6.hsaco \
FE2O3_GFX942_ALPHA_ZETA_SHA256=3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4 \
cargo +nightly-2026-04-03 test --locked -p fe2o3-hsa-runtime \
  --features hardware-test-hooks --test gfx942_two_kernel_hardware \
  gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator \
  -- --ignored --exact --nocapture
```

That test uses checked generated slice capabilities, independent typed
alpha/zeta preparation, safe dispatch, and one reviewed loaded executable. Its
semantic witnesses and prerequisite authenticator are explicit test fixtures.
It is observed runtime-composition and hardware evidence, not production
authentication, a dashboard `remote-hardware` strength, or a parity promotion.

To archive that exact test in an isolated parity snapshot, first place the
HSACO as a regular non-symlink file under the external archive root, then run:

```bash
scripts/run-parity-snapshot.sh run \
  --repo "$PWD" \
  --archive-root /evidence/snapshot-001 \
  --shard Q5 \
  --gfx942-alpha-zeta-hardware \
  --alpha-zeta-hsaco artifacts/alpha-zeta-cov6.hsaco \
  --alpha-zeta-sha256 3a916cdabca05ac74d340889aab2067221d6d1252a7cde13e61c1786252565c4
```

The shard name is `GFX942-ALPHA-ZETA-HARDWARE`. It records the expected digest,
the artifact's independently computed record digest, and the exact test name.
Its name and command preserve the fake-authenticator boundary; the resulting
record does not upgrade production-authentication evidence.

Durable Worker V2 publication, finalized-bundle host admission, currentness
leases, the authenticated HSA load state machine, generated alpha/zeta safe
dispatch SPI, and the reviewed `fe2o3-hsa-runtime` adapter are production code.
The blocking production gaps include durable lease reacquisition, a canonical
Worker V2 load envelope with complete bundle/proof and raw/finalized lineage,
production Cargo publication and application handoff, recovered host admission,
and a production `WorkerV2PrerequisiteAuthenticatorV1`. Current authenticator
implementations are tests/fakes, so compiler, Verus, proof, ABI, and effect
evidence cannot yet be authentically promoted into safe load/launch authority.
Neither adapter nor either hardware observation may restamp a declaration
without an updated shard and archived clean-checkout result record.

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

# Direct LLVM link evidence

These tools produce inert, canonical evidence. They do not publish an artifact,
authorize module loading, grant kernel launch authority, or prove that a record
came from trusted CI. `attestation.py` can authenticate a detached observation,
but it is deliberately not connected to the V3 release gate yet.

## Trust boundary

Reproducibility V3 has two inspection commands:

- `inspect` verifies bounds, canonical encoding, typed identities, expanded argv,
  and record integrity. A structurally valid record may describe a local match,
  failure, or unavailable run.
- `validate` additionally compares independently supplied expected identities,
  but always returns nonzero with
  `missing-authenticated-upstream-attestations`. An internally consistent record
  is forgeable and cannot become a release pass merely by matching CLI values.

The former release-like matrix command is now `inspect-matrix`. It checks the
shape and consistency of the three target observations but grants no release
authority.

## Authenticated attestation V1

`attestation.py` verifies bounded canonical JSON payloads with OpenSSH Ed25519
signatures. It uses both the OpenSSH signature namespace and the signed payload
field `domain=fe2o3-direct-link-attestation-v1` for domain separation.

Each payload binds exactly:

- schema version and one role: `g2-worker`, `g5-publication`, `g6-bundle`,
  `g7-hardware-runner`, or `g7-static-runner`;
- a signer identity, source commit, AMDGPU target, issued-at time, expiration no
  more than seven days later, policy identity and epoch, and a bounded campaign
  nonce; and
- a role-specific, complete, sorted set of typed subject identities. Unknown,
  missing, duplicate, extra, and permuted fields or subjects are rejected.

The `build_context_identity` is derived, not supplied as an opaque build ID. Its
canonical preimage binds the commit, target, role, complete role-specific
subject set, policy identity, policy epoch, and campaign nonce. Verification
recomputes it from independently expected values.

The exact subject-name sets are:

- `g2-worker`: `llvm_toolchain`, `request`, `worker`, `worker_executable`
- `g5-publication`: `linked_artifact`, `publication`, `request`
- `g6-bundle`: `bundle`, `ffi_closure`, `final_artifact`, `publication`
- `g7-hardware-runner`: `argv`, `bundle`, `driver`, `final_artifact`,
  `hardware_run`, `observed_gpu`, `oracle`, `test_executable`
- `g7-static-runner`: `argv`, `bundle`, `final_artifact`, `ruleset`,
  `runner_executable`, `static_run`

Each `(role, subject)` also has an exact identity domain:

| Role | Subject domains |
| --- | --- |
| `g2-worker` | `llvm_toolchain=fe2o3-llvm-toolchain-v1`, `request=fe2o3-link-request-v1`, `worker=fe2o3-worker-v1`, `worker_executable=fe2o3-worker-executable-v1` |
| `g5-publication` | `linked_artifact=fe2o3-linked-artifact-v1`, `publication=fe2o3-link-publication-v1`, `request=fe2o3-link-request-v1` |
| `g6-bundle` | `bundle=fe2o3-bundle-index-v1`, `ffi_closure=fe2o3-ffi-closure-v1`, `final_artifact=fe2o3-final-artifact-v1`, `publication=fe2o3-link-publication-v1` |
| `g7-hardware-runner` | `argv=fe2o3-hardware-argv-v1`, `bundle=fe2o3-bundle-index-v1`, `driver=fe2o3-gpu-driver-v1`, `final_artifact=fe2o3-final-artifact-v1`, `hardware_run=fe2o3-hardware-run-v1`, `observed_gpu=fe2o3-observed-gpu-v1`, `oracle=fe2o3-hardware-oracle-v1`, `test_executable=fe2o3-test-executable-v1` |
| `g7-static-runner` | `argv=fe2o3-static-argv-v1`, `bundle=fe2o3-bundle-index-v1`, `final_artifact=fe2o3-final-artifact-v1`, `ruleset=fe2o3-static-ruleset-v1`, `runner_executable=fe2o3-static-runner-executable-v1`, `static_run=fe2o3-static-run-v1` |

The trust policy is also canonical JSON. It has a positive, bounded policy epoch.
Each entry maps one exact `(role, signer identity, validity interval)` to one
comment-free Ed25519 public key and its derived key identity. Multiple keys may
support rotation for one role/signer only when their half-open validity windows
do not overlap. The key selected for verification must be active at the signed
`issued_at` time. The policy pins both the literal verifier path
`/usr/bin/ssh-keygen` and the typed digest of the verifier bytes.

The caller must supply the expected policy identity and epoch from a separate
trusted configuration; computing an identity from an untrusted policy does not
pin that policy. Rotation creates a new policy with a strictly larger epoch and
requires an out-of-band pin update. Revocation removes the key, increments the
epoch, and requires callers to stop accepting the old policy identity. There is
no network revocation lookup or implicit rollback protection. A key reaching
`valid_until` prevents later issue times but does not invalidate an attestation
issued earlier; payload expiration and current accepted policy determine its
remaining usability.

Verification additionally requires caller-supplied expected values for the
role, signer, source commit, target, policy epoch, campaign nonce, and every
role-specific subject. A valid signature over internally consistent
attacker-chosen values is therefore insufficient. Inputs are opened with
`O_NONBLOCK|O_NOFOLLOW`, checked with `fstat` before reading, and accepted only
as bounded regular files. FIFOs, devices, sockets, symlinks, noncanonical JSON,
duplicate keys, malformed keys and signatures, expired or future observations,
and mismatched replay bounds fail closed.

The verifier opens the exact system path without following a final symlink,
requires a root-owned executable that is not group/other writable, copies the
measured bytes to a sealed `memfd`, checks their policy-pinned identity, and
executes those bytes through `/proc/self/fd`. `ssh-keygen -Y verify` runs without
a shell, with a fixed environment and a timeout. The allowed-signers content,
signature, and payload are sealed `memfd` objects passed as exact
`/proc/self/fd` paths and descriptors; verification creates no temporary paths
and does not consult `TMPDIR`. Stdout and stderr are drained nonblockingly under
one aggregate bound, and the verifier process group is killed immediately on
overflow or timeout.

Useful policy setup commands are:

```console
python3 scripts/direct-link/attestation.py measure-verifier
python3 scripts/direct-link/attestation.py policy-identity policy.json
ssh-keygen -Y sign \
  -f /secure/path/to/signer \
  -n fe2o3-direct-link-attestation-v1 \
  attestation.json
```

`verify` requires `--expect-policy-identity`, `--expect-role`,
`--expect-signer`, `--expect-source-commit`, `--expect-target`,
`--expect-policy-epoch`, `--expect-campaign-nonce`, and one
`--expect-subject name=typed-identity` for every subject in that role.

Success returns `VerifiedAttestationObservationV1`, which is intentionally
forgeable descriptive Python data with no publication, load, launch, or durable
trust capability. A future trusted gate must call `verify_signed_attestation`
itself and consume the result immediately in the same control flow. It must not
accept a serialized, cached, reconstructed, or caller-supplied observation
object as proof that verification occurred.

This primitive is not imported by `reproduce.py` or `evidence.py`. There are no
trusted G2, G5, G6, G7, or static-runner attestation producers yet, so the
default release gate remains blocked.

`reproduce.py run` returning zero means only that this invocation observed equal
linked and finalized bytes in its two builds. The aggregate `evidence.py` gate
remains blocked until it consumes canonical G2 execution, G5/G6 publication and
bundle, G7 hardware, and static-runner attestations.

Every V3 record carries `trust_level=unauthenticated-local-observation`. The
aggregate envelope records even a matching local reproducibility observation as
`unavailable:unauthenticated-reproducibility`, never as a passing release suite.

## Reproducibility V3

`reproduce.py run`:

1. resolves an exact Git commit and hashes its canonical `git ls-tree` snapshot;
2. rejects Git submodules because their checked-out content is not yet bound;
3. creates two independent local clones and detached checkouts with system and
   global Git configuration, attributes, hooks, and filter drivers disabled;
4. hashes the actual checked-out tracked file bytes and symlink targets, and
   retains per-checkout inode, mode, size, mtime, and ctime guards for the
   post-build check;
5. requires `{source_dir}`, `{build_dir}`, and `{target}` placeholders and records
   the canonical template plus both fully expanded argv vectors;
6. copies the measured build executable into a sealed Linux `memfd` and executes
   those pinned bytes through `/proc/self/fd`; there is no unpinned fallback;
7. captures stdout and stderr through bounded parent-owned pipes and kills the
   original process group after success, failure, overflow, or timeout; and
8. measures linked and finalized artifacts relative to an already-open build
   directory, traversing every component with `O_NOFOLLOW`.

Identity hashes contain a versioned magic value, length-delimited identity
domain, payload length, and payload. File measurements reject changes to device,
inode, size, mtime, or ctime while the descriptor is being read.

Example:

```console
python3 scripts/direct-link/reproduce.py run \
  --commit "$COMMIT" \
  --target gfx942 \
  --linked-artifact output/linked.hsaco \
  --final-artifact output/final.hsaco \
  --source-dir "$PWD" \
  --work-root /tmp \
  --llvm-toolchain-identity "$LLVM_TOOLCHAIN_ID" \
  --worker-identity "$WORKER_ID" \
  --request-identity "$REQUEST_ID" \
  -- /absolute/path/to/build-tool \
     --source '{source_dir}' \
     --output '{build_dir}' \
     --target '{target}' > repro-gfx942.tsv
```

## Aggregate evidence

`evidence.py collect` does not accept caller-supplied suite pass statuses. It
consumes the V3 local reproducibility observation and exact release files. The
other suites remain unavailable, so collection emits a canonical blocked record
and returns nonzero.

A future hardware pass must bind the exact commit, link request, finalized
artifact, target, observed GPU and driver, test executable and argv, oracle, and
execution outcome. A digest-shaped string is insufficient.

## Remaining limits

- Process-group cleanup cannot contain a hostile child that calls `setsid`, joins
  another process group, or escapes an external cgroup. Production runs need the
  G2 supervisor plus OS containment.
- Only the top-level build executable is pinned. Compilers, linkers, interpreters,
  shared libraries, and other subprocesses it loads remain assertions until the
  complete toolchain closure is pinned and attested.
- Artifact descriptors are closed after measurement. Same-byte publication and
  runtime loading still require the G5/G6 transaction and bundle path.
- V3 reproducibility and aggregate records are not signed and are not
  trusted-runner attestations. The detached V1 verifier is not release-gate
  wiring; `validate` remains blocked even for an internally consistent matching
  record.
- Linux sealed `memfd` execution and procfs are required. Unsupported hosts fail
  closed instead of running an unpinned executable.
- The policy identity must be distributed and pinned out of band. Signer key
  custody and role assignment remain external operational controls. Rotation
  and revocation are ineffective while any trusted caller still accepts the old
  policy identity or epoch. A signature proves possession of an allowed key,
  not that the producer measured its subjects correctly.
- Expiration and key validity trust the verifier host clock. The derived build
  context prevents cross-context and cross-campaign replay, but an attestation
  can be replayed within its validity window for the same context unless a later
  gate adds a durable one-time-use ledger.
- The `ssh-keygen` file bytes are pinned, but its ELF interpreter, shared
  libraries, kernel, procfs, Python runtime, and host remain in the trusted
  computing base. Production runner attestation must bind that closure or run
  in a separately attested environment.

Run the CPU-only hardening suite with:

```console
python3 -m unittest discover -s scripts/direct-link/tests -v
ruff check scripts/direct-link
ruff format --check scripts/direct-link
```

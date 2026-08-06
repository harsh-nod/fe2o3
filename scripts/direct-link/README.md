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
  more than seven days later, and an exact replay-bounded build identity; and
- a role-specific, complete, sorted set of typed subject identities. Unknown,
  missing, duplicate, extra, and permuted fields or subjects are rejected.

The exact subject-name sets are:

- `g2-worker`: `llvm_toolchain`, `request`, `worker`, `worker_executable`
- `g5-publication`: `linked_artifact`, `publication`, `request`
- `g6-bundle`: `bundle`, `ffi_closure`, `final_artifact`, `publication`
- `g7-hardware-runner`: `argv`, `bundle`, `driver`, `final_artifact`,
  `hardware_run`, `observed_gpu`, `oracle`, `test_executable`
- `g7-static-runner`: `argv`, `bundle`, `final_artifact`, `ruleset`,
  `runner_executable`, `static_run`

The trust policy is also canonical JSON. Each entry maps one exact
`(role, signer identity)` pair to one comment-free Ed25519 public key. It pins
both the literal verifier path `/usr/bin/ssh-keygen` and the typed digest of the
verifier bytes. The caller must supply the expected policy identity from a
separate trusted configuration; computing an identity from an untrusted policy
does not pin that policy.

Verification additionally requires caller-supplied expected values for the
role, signer, source commit, target, build identity, and every role-specific
subject. A valid signature over internally consistent attacker-chosen values is
therefore insufficient. Inputs are bounded regular files opened with
`O_NOFOLLOW`, and the verifier rejects noncanonical JSON, duplicate keys,
malformed keys and signatures, expired or future observations, and mismatched
replay bounds.

The verifier opens the exact system path without following a final symlink,
requires a root-owned executable that is not group/other writable, copies the
measured bytes to a sealed `memfd`, checks their policy-pinned identity, and
executes those bytes through `/proc/self/fd`. `ssh-keygen -Y verify` runs without
a shell, with a fixed environment, bounded inputs and output, and a timeout.
The allowed-signers and signature files are private temporary copies made from
the already-read policy and signature bytes.

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
`--expect-build-identity`, and one `--expect-subject name=typed-identity` for
every subject in that role. Success creates only an
`AuthenticatedObservationV1`; the type has no publication, load, or launch
capability.

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
  custody, revocation, rotation, and role assignment are external operational
  controls. A signature proves possession of an allowed key, not that the
  attested producer measured its subjects correctly.
- Expiration trusts the verifier host clock. The exact expected build identity
  prevents cross-build replay, but an attestation can be replayed within its
  validity window for that same build unless a later gate adds a durable
  one-time-use ledger.
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

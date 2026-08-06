# Direct LLVM link evidence

These tools produce inert, canonical evidence. They do not publish an artifact,
authorize module loading, grant kernel launch authority, or prove that a record
came from trusted CI.

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
- Records are not signed and are not trusted-runner attestations. `validate`
  therefore remains blocked even for an internally consistent matching record.
- Linux sealed `memfd` execution and procfs are required. Unsupported hosts fail
  closed instead of running an unpinned executable.

Run the CPU-only hardening suite with:

```console
python3 -m unittest discover -s scripts/direct-link/tests -v
ruff check scripts/direct-link
ruff format --check scripts/direct-link
```

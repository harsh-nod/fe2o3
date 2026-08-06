# Direct LLVM link release evidence

These tools produce inert, canonical evidence. They do not publish an artifact,
authorize module loading, grant kernel launch authority, or prove that a claim
was produced by trusted CI.

## Fail-closed release gate

V2 separates structural inspection from release validation:

- `inspect` verifies bounds, canonical encoding, typed identities, and record
  integrity. It returns success for structurally valid pass, fail, or blocked
  records.
- `validate` verifies the caller's pinned expectations and returns success only
  for a passing gate. A valid blocked or failed record returns nonzero.

`evidence.py collect` no longer accepts caller-supplied suite pass statuses. It
consumes a V2 clean-build runner record for reproducibility. Compile, direct-link,
hardware, and static suites remain unavailable until canonical G2/G5/G6/G7 and
static-runner record parsers are connected. Therefore the aggregate release gate
is truthfully blocked even when reproducibility passes.

A future hardware pass must consume a canonical runner record that binds the
exact commit, link request, finalized artifact, target, observed GPU and driver,
test executable and argv, oracle, and execution outcome. A digest-shaped string
or CLI status is insufficient.

## Reproducibility V2

`reproduce.py run`:

1. resolves an exact Git commit and hashes its canonical `git ls-tree` snapshot;
2. creates two independent local clones and detached checkouts of that commit;
3. verifies each checkout is clean before and after its build;
4. executes an absolute build executable directly, without a shell, in two new
   build directories;
5. binds canonical argv, executable bytes, a fixed environment, Git executable,
   LLVM/toolchain identity, worker identity, request identity, and target; and
6. measures both the linked and finalized artifacts under distinct identity
   domains.

Identity hashes include a versioned magic value, the length-delimited identity
domain, the payload length, and the payload. Files are opened once with symlink
following disabled and rejected if their device, inode, size, modification time,
or change time changes during measurement.

Stdout and stderr are captured through bounded parent-owned pipes. The log bound
does not limit object or HSACO sizes. The runner creates a process group and kills
that group after every outcome, including normal parent exit and timeout. It does
not contain a hostile process that creates a new session or escapes into another
process group; production gating needs the G2 supervisor plus OS containment,
such as a delegated cgroup, for that threat model.

The executable and artifacts are measured evidence, not same-descriptor runtime
authority. A later publisher or loader must consume the typed G5/G6 bundle and
load the exact admitted bytes. LLVM/toolchain, worker, and request identities are
bound assertions until their canonical upstream records are parsed.

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

The former existing-file `compare` command was removed because comparing two
caller-selected paths cannot satisfy clean-build release gating.

## Evidence collection

Collection consumes the exact release files and the V2 reproducibility result:

```console
python3 scripts/direct-link/evidence.py collect \
  --git-commit "$COMMIT" \
  --target gfx942 \
  --worker-executable "$WORKER" \
  --worker-identity "$WORKER_ID" \
  --llvm-toolchain-identity "$LLVM_TOOLCHAIN_ID" \
  --request-identity "$REQUEST_ID" \
  --linked-artifact "$LINKED_HSACO" \
  --final-artifact "$FINAL_HSACO" \
  --repro-result repro-gfx942.tsv > evidence.tsv
```

This currently returns nonzero with a canonical blocked record. That is expected
until the remaining typed provenance consumers exist.

Run the CPU-only hardening tests with:

```console
python3 -m unittest discover -s scripts/direct-link/tests -v
ruff check scripts/direct-link
ruff format --check scripts/direct-link
```

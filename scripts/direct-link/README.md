# Direct LLVM link release evidence

These tools produce inert, canonical evidence for the direct LLVM linking
milestone. They do not publish an artifact, authorize module loading, or grant
kernel launch authority.

## Evidence record

`evidence.py collect` hashes the worker executable and linked artifact and emits
a bounded V1 TSV record. It also binds the exact Git commit, AMD target, worker
build identity, LLVM package/build identity, request identity, suite outcomes,
and an optional hardware execution identity. Every non-passing suite must carry
an explicit bounded reason code.

Files are opened once with symlink following disabled, checked as regular and
bounded, hashed through that descriptor, and rejected if their device, inode,
size, modification time, or change time differs after measurement.

The required suites are:

- `clean-build-reproducibility`
- `compile`
- `direct-llvm-link`
- `hardware-execution`
- `static-checks`

The release gate is derived from those outcomes. It is `pass` only when every
suite passed. A hardware pass additionally requires a
`fe2o3-hardware-v1-sha256-*` execution identity; compile-only evidence cannot
satisfy that requirement.

```console
python3 scripts/direct-link/evidence.py collect \
  --git-commit "$COMMIT" \
  --target gfx942 \
  --worker-executable "$WORKER" \
  --worker-build-id "$WORKER_BUILD_ID" \
  --llvm-build-identity "$LLVM_BUILD_ID" \
  --request-identity "$REQUEST_ID_HEX" \
  --artifact "$HSACO" \
  --hardware-execution-identity "$HARDWARE_EXECUTION_ID" \
  --suite clean-build-reproducibility=pass \
  --suite compile=pass \
  --suite direct-llvm-link=pass \
  --suite hardware-execution=pass \
  --suite static-checks=pass > evidence.tsv
```

Release validation requires authenticated CI expectations and the actual worker
and artifact files. It also checks that a canonical reproducibility record
matches the target, suite outcome, and artifact digest.

```console
python3 scripts/direct-link/evidence.py validate evidence.tsv \
  --expect-commit "$COMMIT" \
  --expect-target gfx942 \
  --worker-executable "$WORKER" \
  --expect-worker-build-id "$WORKER_BUILD_ID" \
  --expect-llvm-build-identity "$LLVM_BUILD_ID" \
  --expect-request-identity "$REQUEST_ID_HEX" \
  --artifact "$HSACO" \
  --repro-result repro-gfx942.tsv
```

## Reproducibility records

`reproduce.py run` executes one argv command in two newly created build
directories. It uses a fixed locale, timezone, source-date epoch, and exact
target. The command is executed directly without a shell. The placeholders
`{build_dir}`, `{source_dir}`, and `{target}` are replaced in each argument.

```console
python3 scripts/direct-link/reproduce.py run \
  --target gfx942 \
  --artifact output/kernel.hsaco \
  --source-dir "$PWD" \
  --work-root /tmp \
  -- cmake --build '{build_dir}' --target direct-link-gfx942 \
  > repro-gfx942.tsv
```

Existing artifacts can be compared with `reproduce.py compare`. A complete
release matrix requires canonical passing base-target records for `gfx1151`,
`gfx942`, and `gfx950`:

```console
python3 scripts/direct-link/reproduce.py matrix \
  repro-gfx1151.tsv repro-gfx942.tsv repro-gfx950.tsv
```

Run the CPU-only tooling suite with:

```console
python3 -m unittest discover -s scripts/direct-link/tests -v
```

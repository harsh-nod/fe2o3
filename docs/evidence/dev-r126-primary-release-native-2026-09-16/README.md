# R126 Packetless Native Release Probe

Development evidence only. R125 remains the accepted Native checkpoint; this
probe is not an R126 qualification campaign, runtime-facade test, formal proof,
memory-bound demonstration or HIP/HSA benchmark.

## Execution

On 2026-09-16 UTC, the primary agent built the `kfd-compute-aql-queue` example
locally from development base `43746b71889cc03778dc1b945a7d86788e6b5ccb` plus
the accompanying `source.patch`. The final patch includes subsequent test-only
assertion additions; no non-test library or example source changed after this
native binary was built. The source patch is development provenance, not a
closed qualification source map.

The patch uses zero context to avoid whitespace-only context lines in the
artifact. Apply it to the named base with `git apply --unidiff-zero source.patch`.

Build command:

```sh
cargo build --locked --offline -p fe2o3-kfd --features live-validation --target x86_64-unknown-linux-musl --example kfd-compute-aql-queue
```

Compiler: `rustc 1.96.0-nightly (55e86c996 2026-04-02)`, full compiler commit
`55e86c996809902e8bbad512cfb4d2c18be446d9`, LLVM 22.1.2.
Cargo: `1.96.0-nightly (888f67534 2026-03-30)`.
The executable is an x86-64 static PIE; no build ran on the shared GPU host.

Executable SHA-256, independently matched locally and after upload:
`f13a1b2dba609fc4c81eb0be183acfe32b709f21d0597715c71cfc9dd1084aa4`.

Host: SSH alias `mi300x`, hostname `sharkmi300x-1`. ROCm SMI reported zero GPU
utilization on all eight devices immediately before the probe; that observation
does not reserve the shared host. Only explicit device unique ID
`0x6ced1647a296545c` was selected.

Remote command:

```sh
timeout --signal=TERM --kill-after=10s 60s /tmp/fe2o3-r126-primary-release-20260915.oBaTCZU6/kfd-compute-aql-queue --retained-release 0x6ced1647a296545c
```

The SSH command completed with exit code zero, not a timeout. The example's
isolated child also exited normally with code zero. `native-output.log` retains
the complete combined output, including both the post-Drop marker and the
normal queue report. There was no fallback or retry.

## Scope

The actual constructor creates an ordinary 4096-byte AQL queue with its original
event, shadow mappings, runtime owner, doorbell, completion signals and queue
resource authority. The example requires retained-profile admission and borrowed
preflight, transfers the exact queue into `PrimaryQueueReleaseCustodyV1`, and
checks the returned queue ID and five released resources. Only after explicitly
dropping the completed public root does it print the success marker.

This is genuine no-dispatch queue creation and retained destruction on one
MI300X, with zero packets and zero MMIO stores. It covers the concrete completed
public-root Drop boundary. It does not qualify native failure retention, dispatch
teardown, runtime-facade accounting/events, multi-device behavior, formal
correspondence, aggregate memory or performance.

## Cleanup

The task-owned staging directory was created by `mktemp -d` with mode 0700.
It contained only the 68,088,648-byte executable. After authoritative command
completion, an anchored process search found no probe/timeout process using that
path. The executable was removed by exact pathname, `rmdir` removed the private
directory, and `test ! -e` confirmed its absence. All cleanup SSH commands exited
zero. No remote source tree, build cache, log, executable or task job remains.

Raw build and native output logs are retained alongside this receipt. A digest
manifest binds these logs and the source patch; this small development receipt
does not substitute for the remaining independent R126 evidence campaign.

# Native Startup Bounds And Activation Mechanics

Date: 2026-09-26. Continuation of the
[root-admission checkpoint](conditional-native-root-admission-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 production-to-safe-GPU-launch completion is
claimed. Installed startup and provisioning still use V1.**

Base: `cbcd1e5bbd38d5678d639d9f916092c1683ef513`.
Admission bounds: `148eec4ef47ad2176000063d35d3a8e1a0f61769`.
Activation worker integration: `255400be0`.
Compiled implementation: `864c371d914f42128ff97943aec1382a1cb18fd1`.

## Admission Bounds

`InheritedCompilerExecutionDeploymentV2/V3::admission_quota()` now returns a
complete conservative logical work and additional-peak bound for admission.
Unlike `SOURCE_STORAGE`, it includes nested decoding, capability construction,
three leases, root joins, listener/service inputs, source provenance checks,
zeroizing seed reads, key construction, trust binding and anchor preparation.
Transient peaks are deliberately summed conservatively with retained outputs.
This is not a bound on elapsed time, generated stack, allocator overhead or RSS.

`ProtectedStaticExecutableV2::quota_for_length` shares the actual executable
quota calculation without requiring a placeholder digest or admitted owner.
Zero length and checked-arithmetic overflow refuse. The anchor's new
`maximum_preparation_quota` uses the same preparation calculation at the native
family's fixed helper/daemon ceilings. These queries perform no descriptor or
secret access and grant no authority.

The original request account still meters every operation. The query neither
creates an account nor resets work, storage peaks or first-denial history. It
does not fund consuming launch, monitoring or the independent cleanup pool.
Complete startup/cleanup funding remains a separate composition task.

## Activation Mechanics

The private `native_activation` module is compiled but is not the installed
entrypoint. Capture requires a unique unsafe single-threaded startup contract.
It reads `/proc/self/cmdline` with one bounded positional read and an EOF probe,
and accepts exactly one nonempty NUL-terminated argument. The complete command
line including its terminator is at most 4096 bytes.

The raw C environment is snapshotted with at most 256 entries, 4096 bytes per
entry including its terminator, and 64 KiB total. It does not use unbounded
`std::env` copies or C-string length scans. Duplicate or missing activation
variables refuse. PID text must be canonical positive decimal naming the main
process; descriptor count and all fourteen ordered roles must match exactly.
Readiness uses a bounded absolute filesystem or abstract Unix address. The
environment is cleared once, only after successful validation.

Readiness is one nonblocking exact `READY=1` datagram attempt, checked against
the captured PID and main-thread ID. A funded failed attempt cannot be retried.
Termination handling owns a thread-affine blocked SIGTERM/SIGINT mask, performs
one finite `sigtimedwait` per interval, and treats EINTR/EAGAIN as empty ticks.
Restoration is explicit and metered. Drop does not restore the mask; failed
restoration requires retaining the owner or terminating the dedicated process.
The runner must not unblock termination before cleanup finishes.

All operations return to entry storage without refunding work. Returned owners
carry full unreserved storage charges; callers must reserve them before retention.
The unsafe environment/descriptor/signal contracts are caller obligations, not
facts established by parser tests or by comparing PID/TID values.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time, and compiled inputs frozen during each run. One native worker
implemented activation privately and independently reviewed admission accounting;
the primary integrated changes and ran all commands. No Qwen worker was used.

| Check | Result |
| --- | --- |
| Three-crate GNU unit suite, no filters | 200 reported passes, 9 failures, 0 ignored; exit 101 |
| New activation tests | 28 passed on GNU and musl |
| Three-crate doctests | 18 positive and 126 compile-fail passed; exit 0 |
| Fifteen-package all-target check | Passed with existing warnings; exit 0 |
| Full coordinator musl release suite | 122 reported passes, 8 failures, 0 ignored; exit 101 |
| Existing V1 static coordinator gate | ELF inspection and two fail-closed activation smoke cases passed; exit 0 |
| Changed Rust formatting | Passed |

GNU totals are coordinator 122/8, anchor coordinator 63/1, static executable
15/0. Nested subprocess output is not counted twice. Failures remain eight
socket EPERM and one ACL fixture EINVAL; musl retains the coordinator's seven
EPERM and one EINVAL. No test was skipped or relaxed to hide those failures.
The older root-dependent guard fixture explicitly skips under uid 1000 while
libtest reports a pass, so its privileged body receives no validation credit.

Activation tests exercise parsers, bounded snapshots, state transitions and
callback-injected accounting/failure paths, not live environment clearing,
signal installation or notification by a systemd-launched main process.
The root-envelope test reserves the calculated quota; it does not execute
complete successful root admission. The independent source-only review found
no concrete quota issue and explicitly recorded that validation limitation.
Initial builds caught an incorrect helper-limit import and test assertions
requiring Debug on an opaque ledger identity; both were corrected.

The static gate still builds the V1 entrypoint. It checks ELF64 ET_EXEC, no
dynamic loader/dependencies or undefined symbols, a non-executable stack, and
refusal of missing activation metadata or a forbidden argument. It does not
exercise native activation, protected image admission or successful root boot.
Its executable SHA-256 is
`f931a9c4e4a8b69e16e1f5db194b9dcfcd1b216768bd74ec8ba018489ee99a80`.

Logs are retained in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
c5473635675de66034671ba345c7816e6c16bafb10732ddd9f5b83f8d7dacc5e  conditional-native-startup-activation-three-r2.log
056bd7a3b4bf019d3b179861f0dccd2bc624887dd0eb96697695c06326377127  conditional-native-startup-docs-r1.log
8383c56e5a88c29ffc3d4f72a48350fe8e498ff381fa3ee749e0b2132ce86cbb  conditional-native-startup-all-targets-r1.log
9df0f0e293756826543f33905b8300925d9c9fce8de43612d8f0e3a6d3c9130d  conditional-native-startup-musl-r1.log
d814c8658111ccc39071f030477b2f9f4e5d6f4c5783dce8dcfbe9388f8ae054  conditional-native-startup-static-r1.log
```

## Remaining Gates

1. Compose complete anchor/compiler launch, retained-output and persistent
   cleanup funding before startup, without fabricating admitted owners or
   renewing accounts between stages. Include finite cumulative monitoring,
   cancellation, cleanup draining and signal restoration.
2. Integrate the native runner with matching V3 provisioning records, client
   profile, protected images and system-manager descriptor paths. Changing
   `main.rs` alone would mix incompatible deployment formats.
3. Execute genuine root-owned admission and launch tests, exact/short budgets,
   substituted sources, failure/unwind cleanup and protected static boot.
4. Complete downstream compiler/proof/publication/host integration and run all
   47 tutorials through the single production safe-launch path on the intended
   targets. Compilation and component tests are not this evidence.

Fresh SSH attempts to mi350, mi350-2 and mi300x failed DNS resolution. No remote
job or scratch was created. Publishing both main branches and the issue must
be independently confirmed.

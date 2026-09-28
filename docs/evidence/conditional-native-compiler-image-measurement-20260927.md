# Shared Compiler Image Measurement

Date: 2026-09-27. Follow-up to the
[native publication continuation](conditional-native-compiler-publication-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
Base: `da02b4e162af2f8e7f3dedd0cea89186063e5e82`.

**This is partial compiler-path integration, not completion of an M0-M7
milestone, protected startup, or any additional end-to-end GPU kernel. Production
selectors remain unchanged.**

## Implementation

`fe2o3-process-identity` owns a shared streaming SHA-256 engine for compiler
executables, backend DSOs and its existing retained-source measurements. The
backend and native supervisor no longer maintain separate image hashing loops.
No new dependency, compiler route, artifact family or launch authority is added.

The path API bounds and checks the complete path before opening. The descriptor
API pins an already opened inode and preserves its file offset. Both require a
nonempty bounded regular file; executable measurement also requires execute
permission bits. Hashing uses at most one positional read per 64 KiB chunk,
followed by an EOF probe and a matching final metadata snapshot. Short reads,
interruptions, growth and observed metadata changes refuse without retry.
Legacy process-identity calls now use this conservative short-read policy too;
successful digest bytes are unchanged.

The measurement callback charges before path inspection, open, metadata and
payload operations. For a file of length `n`, payload prepayment is
`n + (ceil(n / 65536) + 2) * 65536`: one logical unit per byte hashed, plus a
fixed allowance per chunk read, EOF probe and final metadata query. Opening and
initial metadata are charged separately. The exported fixed scratch allowance
excludes the caller's path, callback and result/error owners. Native callers
reserve scratch with the existing original-account scope; cleanup restores live
storage without refunding accepted work or clearing peaks and first denials.
These are logical work/storage and syscall-attempt bounds, not instruction,
wall-clock, filesystem latency, generated-stack or process-RSS guarantees.

Native compiler publication now charges both image streams to the client's
original budget. The supervisor retains its additional before/after owner,
group and link-count predicate and propagates resource errors without converting
them to identity failures. Its enclosing observation still pays initial
descriptor/metadata inspection. The engine's digest is only a point observation:
it does not authenticate a pathname, mapped loader image, future contents or
protected custody.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time with compiled inputs frozen. Native workers were unavailable
at the thread limit; the primary agent implemented and tested this change. No
Qwen worker was invoked.

- Process-identity all-target tests: 14 unit and 3 integration tests passed.
  Nested expected-failure receiver subprocesses are not top-level failures or
  additional passes.
- Backend invocation tests: 16 passed, including four new exact/one-short work,
  input-floor, scratch-floor, I/O failure and original-ledger checks.
- Native supervisor image tests: 2 passed, covering retained snapshot/digest
  agreement and payload refusal on the original account.
- Existing native compiler handoff/publication regressions: 12 passed.
- Three-crate all-target compilation check passed for process identity, the
  backend and the authority service, with existing and unselected-path warnings.
- Changed Rust formatting and whitespace checks passed.

Seven new shared-engine tests cover chunk boundaries, cursor preservation,
path replacement, execute-mode policy, all callback refusal stages, malformed
paths, nonregular/empty/oversized files, deterministic shrink/grow/mode mutations,
and borrowed-owner survival on callback unwind. No new test ignores were added.
The first backend test build failed because the new fixture used an unavailable
direct `rustix` dependency; it was corrected to the existing libc pattern before
the passing rerun. Full privileged socket/service execution was not rerun here.

Logs are retained in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
4a18bf53345532ad5f38084756f82ffc97deaf6452d8b317ae6391d1f59bf32d  compiler-image-process-identity-20260927.log
959d7c05f42013df34d99fc8ae6b17b496f909502e1bfbec1ed8d1d0d66d404a  compiler-image-backend-r2-20260927.log
462bba498cf48dfbed21dd2eace26eaec401c4a033d2ec3ce0574b6a7f3ee105  compiler-image-supervisor-20260927.log
791b7da22c79bbedf7e51acb9dd3634c97c4067add7418604d095c4c485b7745  compiler-image-publication-regressions-20260927.log
d8a3ca84372c31f15c19a034c0ca9c72ca2220f3e58b0a8d1fa383559dc193f0  compiler-image-all-targets-20260927.log
```

## Remaining Gates

Complete live argv, canonical cwd, current environment and legacy capability
admission/revalidation still need bounded native integration. In particular,
`std::env::vars_os()` captures the current environment before a caller can bound
its allocation. `/proc/self/environ` describes the initial environment and is
not a semantics-preserving substitute. Image metering does not close that gap.

Cargo profile/policy custody, finalization and safe host consumers, coherent
deployment/qualification activation, protected service/compiler tests, and the
full target-matched 47-kernel matrix remain open as described in the preceding
checkpoint. No source-only deployment worker candidate is activated here.

All three SSH probes and the Git fetch failed DNS resolution. GitHub issue reads
succeeded and confirmed #272 is still open with every milestone box unchecked.
No remote job or scratch directory was created; other worktrees and evidence
were preserved.

# Native Policy Child Inheritance

Date: 2026-09-27. Follow-up to the
[compiler image checkpoint](conditional-native-compiler-image-measurement-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
Base: `6f9995fd7d45767081b6e24337c817fea4b3e23d`.

**No complete M0-M7 milestone, native Cargo/compiler activation, protected boot,
or additional end-to-end GPU kernel is claimed. Installed selectors are unchanged.**

## Shared Contract

Both nominal native policy capabilities now expose `inherit_for_child`. They
use one implementation and the same descriptor reservation/post-fork checks as
the existing sealed-capability inheritance path. This is the missing parent-side
operation needed to migrate Cargo without embedding another copy of descriptor
transport inside Cargo or the diagnostic executable. It introduces no compiler
route, kernel-name selection, policy downgrade or execution authority.

The caller prepays the policy's full retained storage plus `FILE_STORAGE` on its
original account before calling. Admission charges `IO_WORK` and reserves
`IO_STORAGE` scratch before inspection. Work, peak storage and first-denial history
are preserved; scratch is restored to the entry floor on return or unwind.

The shared helper refuses occupied FD 202 and uses non-replacing
`F_DUPFD_CLOEXEC` reservation. A concurrent claimant is never overwritten.
`Command` then owns the exact alias until drop. After fork, the hook checks seals,
CLOEXEC, mode, length, device and inode before clearing CLOEXEC in the child only.
It uses descriptor syscalls and scalar comparisons, without decoding, allocating
or retrying. No fallible capability operation follows hook installation.

The alias reservation remains caller-owned until `Command` drops, including after
a failed spawn. Dropping the original policy does not invalidate the command's
alias. Native callers must drop the command after its one intended spawn.
The API does **not** budget arbitrary `Command` construction, hook-vector backing
and growth, other hooks, spawning, child execution or elapsed time. Those require
separate caller accounting. Public sealed policy transport still requires
independent policy provenance and does not establish protected compiler custody.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time. Source was frozen during compilation. Native agent creation
again hit the thread limit; the primary performed implementation and review.
No Qwen worker was used.

- Full capability all-target suite: 205 unit tests plus 22 external-anchor and
  16 supervisor integration tests passed. No failures or ignored tests.
- Capability doctests: 10 positive and 120 compile-fail tests passed.
- Six new top-level tests exercise both V2 and V3 in isolated subprocesses.
  Their nested libtest invocations are not counted again.
- Real child exec admits the exact policy bytes from non-CLOEXEC FD 202 after the
  parent policy is dropped. Parent flags remain CLOEXEC and command drop closes
  the alias.
- Exact quotas succeed; one-short input, work and scratch refuse before slot
  mutation. Occupied slots remain owned by their original file. Failure leaves
  the command usable, with no partially installed hook.
- Failed exec, parent unwind, a child hook clearing CLOEXEC, and same-byte sealed
  inode substitution preserve parent ownership and reject at the intended boundary.
- Existing legacy inheritance tests pass through the shared implementation.
- All-target compilation passed for `cargo-fe2o3`, `rustc-codegen-fe2o3` and
  `fe2o3-compiler-execution-client`, with existing/unselected-path warnings.
- Changed Rust formatting and whitespace checks passed. No remote job or files
  were created; the private local test scratch was empty after validation.

Logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
a56f516a83863d3fa2def75541608941f18a7a887259fc452302d1feeac7e03b  native-policy-inheritance-tests-r2-20260927.log
4d5813a18fd830534fb29c64b32cf03729868e79243035789ebede1a7163ecb7  native-policy-inheritance-docs-20260927.log
e1fae1cf8b34f3e0a2cc706452d8e0b19540cf47993904b85bbeebda3cbb12dd  native-policy-inheritance-consumer-check-20260927.log
```

## Next Integration

Cargo still retains V1 profile/policy, readiness and receipt types. Migrating its
broker custody, authority release, compiler handoff/finalization and host consumers
must accompany the V3 deployment switch; changing the service inventory alone
would leave mismatched production consumers.

The isolated native client-check candidate `40e7fc62a` can replace its private
policy-inheritance implementation with this API, but remains unmerged. Review also
found its child admits the policy before checking both inherited slots. It must
reuse the compiler's paired FD 202/195 preflight before any duplication, so a
missing service slot cannot become a private policy descriptor. The separate
inventory candidate `9d86b16f4` remains unselected.

Complete bounded live-process capture, native Cargo consumers, coherent deployment
qualification, protected execution and the target-matched 47-kernel matrix remain
required. This checkpoint validates public policy transport on the local OS, not
root-protected issuer startup or GPU execution.

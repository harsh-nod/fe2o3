# Native Compiler Publication Continuation

Date: 2026-09-27. Follow-up to the [parent handoff checkpoint](conditional-native-client-handoff-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
Base: `185976afca59f970c084cff180464965a176c1f7`.
Compiler continuation: `5d2020bb818f6cb6a2f83b53819fdd6f80471cc2`.
Native recovery: `e4df44a475d8278f3b78937633577da2b031c162`.

**No complete M0-M7 milestone, activated production-native pipeline, protected
boot or 47/47 GPU result is claimed. The installed selectors remain unchanged.**

## Implemented

The compiler now has an unselected native continuation over the existing
importer, analysis and conditional prefix. It retains the complete original
source/target preparation while publishing its V5 handoff, deriving the exact
V3 subject, acquiring a native receipt and publishing that receipt's transport.
The native client's exclusive original-account borrow spans preparation through
transport. It does not create a second importer or manufacture proof authority
from serialized bytes.

Policy FD 202 and service FD 195 have consuming admission and single-closer
cleanup. Independent review found that duplicating the policy before checking
an absent service slot could assign the private policy File to FD 195 and cause
a double close. Admission now prepays inspection and checks both inherited slots
before any duplication. An isolated regression uses a real sealed V3 policy,
fills free descriptors below 195 without overwriting live owners, proves 195 is
the next duplicate, and checks refusal before policy admission. The original
sealed policy remains valid afterward.

Publication compares the full canonical subject, not just caller-supplied
identity fields. Comparisons and fixed returned metadata are charged; receipt
metadata is prepaid before transport publication. Resource refusal after an
inert filesystem publication can still leave recoverable transaction state.
Neither the handoff nor transport receipt alone grants launch authority.

The shared native client state machine now exposes terminal `recover_only` for
both V2 and V3. It either returns the exact existing carriage or acknowledges
Cancel at the unchanged absence sequence and rollback anchor. It never enters
Inspect/Prepare/Issue/Publish. The returned full output charge is unreserved;
consumed input charges remain caller-owned. Policy/subject comparisons use the
same implementation as ordinary native acquisition, without legacy fallback.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time with compiled inputs frozen. A native worker supplied isolated
deployment/client-check candidates and reviewed the primary changes. No Qwen
worker was used.

- Native compiler handoff/publication tests: 12 passed, including independently
  resealed occurrence/content substitutions and work refusal before comparison.
- Protected-custody selection: 17 passed, 1 existing protected-runtime ignore.
  The isolated descriptor regression is included; its child is not double-counted.
- Native preparation/cancellation tests: 30 passed across both families, including
  exact response kind, sequence, anchor and work-denial checks on native records.
- The final full client run exits 101: 40 library passes and 12 failures; 6 binary
  passes; 6 integration passes, 42 failures and 2 existing child-role ignores.
- Both new public recovery transcript tests compile for each family but fail
  socket-timeout fixture setup with EPERM. Existing socket/credential tests also
  fail at sandbox-restricted operations. No new skip/ignore conceals failures;
  successful real socket exchange is not claimed.
- All-target checking passes for `rustc-codegen-fe2o3` and
  `fe2o3-compiler-execution-client`, with existing and unselected-continuation
  warnings. All 24 client doctests pass. Changed Rust formatting, whitespace,
  source-hygiene delta and the two implementation commits' DCO checks pass.

Logs and SHA-256 identities are retained in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
e576970b1852be1336c4861e34165995b9c16e5173847fdca59e278aa5bd9075  conditional-compiler-native-publication-final-tests.log
e2da6bc23a6721fdb7fd255b7614a2f671ec68a49d66919ddce51b1e99601b9e  conditional-compiler-native-custody-tests-r2.log
e15860ec6a8739ca4688027c1969dd366fcf383ff23ca70f538f6c55e2aeb9f0  conditional-native-recovery-preparation-tests-r2.log
463bf4b06b4cb7cc998bfb4c5fbb2405bdacb04fd33ba8424781f54732542c3c  conditional-native-recovery-client-all-targets-final.log
0a45661d1c472716f7319a5214c8f241b1f552539899ab9313fadad77bd98472  conditional-compiler-native-publication-all-targets-r2.log
d9724542ea40dcdfca2e0cf3a60714b65025e82b15d9cf36cc579abba9ac31f4  conditional-native-recovery-docs-final.log
```

## Activation Gates

1. Replace the legacy unmetered live-rustc observation used by publication with
   the bounded native observation boundary. The new continuation is deliberately
   not selected in `rustc-codegen-fe2o3/src/lib.rs` until this is complete.
2. Migrate Cargo profile/policy custody, readiness, authority release and actual
   V5 handoff/V3 receipt consumers through conditional finalization and safe host
   launch. Current production Cargo consumers still expect the older family.
3. Join deployment inventory, qualification, client-check and root entrypoints
   coherently, preserving lifecycle locks, fourteen activation roles, state roots,
   signing-key continuity and explicit legacy-journal handling.
4. Execute genuine protected service/compiler tests and the full 47-kernel GPU
   matrix. Local unit tests and inert diagnostic fixtures do not satisfy this gate.

Two source-only worker candidates remain isolated, not integrated or enabled:
`9d86b16f484ea849fbfbd17755011d535554bdb8` (deployment inventory) and
`40e7fc62ad4c5e3bd9b6cdb9428c0d34becfa407` (client-check/qualification).
The latter's missing native recovery API is supplied by this checkpoint; it still
needs dependency integration, compilation and production-consumer validation.

SSH probes to `mi350`, `mi350-2` and `mi300x`, and a GitHub fetch, failed DNS
resolution. No remote job or scratch directory was created. Existing source
worktrees, evidence and shared build cache are preserved.

# Native Cargo Readiness and Receipt Custody

Date: 2026-09-27. Follow-up to
[native policy inheritance](conditional-native-policy-inheritance-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
Base: `282867ef283c49ba81794832e3c0496fffa409bc`.

**Preparatory integration only. No complete M0-M7 milestone, production version
switch, protected service boot, or additional end-to-end GPU kernel is claimed.**

## Implemented Boundary

Cargo now has a move-only native readiness owner in its existing compiler
execution boundary. It retains the sealed V3 profile and policy, the exact
selected child and received V3 supervisor readiness, and an exclusive mutable
borrow of the original resource budget. It uses the existing child-channel
handoff; it does not create another compiler or artifact route.

The owner checks the complete profile/policy bytes and child PID before transfer.
Readiness checks include the selected child, distinct client/supervisor UIDs,
external anchor identity, native policy binding, and exact readiness/manifest
binding. Immutable native record construction already enforces canonical framing;
these contextual checks do not clone and decode those records again.

After child completion, publication acquisition uses the existing V5 recovery,
currentness lease and locked token on that same retained account. Receipt
admission reconstructs SubjectV3 from the locked publication, recovers the native
sidecar, decodes and verifies CarriageV3, compares the complete policy and subject,
and rechecks readiness and locked currentness before returning. Returned lease,
token and carriage owners remain fully charged. Temporary storage is retired on
success, refusal or unwind; consumed work, peak and denial history remain.

This boundary does not derive or authenticate broker configuration, prepay the
enclosing Cargo attempt's path/producer/Command storage, prove live compiler
invocation capture, or grant publication/load/launch authority. Those remain
explicit caller and sealed-verifier obligations. A public sealed profile and a
valid signature alone are not proof of protected execution.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time, with compiled inputs frozen during each build.

- Four new portable tests passed, none ignored. They exercise exact seals and
  readiness context, child/UID/anchor/policy/readiness substitution, complete
  signed receipt matching, subject/policy substitution, corrupt/truncated and
  downgraded receipt framing, and explicit absence of authority.
- Exact input/work/storage budgets succeed; one-short budgets reject while
  preserving the original work ledger, entry storage and denial history. Receipt
  decoding returns the full owner charge and does not retain hidden scratch.
- The broader Cargo boundary suite reported **7 passed, 2 failed**, not a clean
  run. The two existing socket-backed tests failed with `InvalidServicePeer` and
  `ESTALE`. A direct AF_UNIX/SOCK_SEQPACKET probe confirmed `SO_TYPE`,
  `getsockname`, and `getpeername` return `EPERM` here; the existing validation
  code maps these refusals to those errors. No socket validation was weakened.
- All-target checks passed for `cargo-fe2o3`, `rustc-codegen-fe2o3`, and
  `fe2o3-compiler-execution-client`, including the new owner and locked intake
  methods. Existing and unselected-path warnings remain.
- The portable tests exercise the shared validation routines, **not** a live
  `finish -> acquire_current_publication -> admit_current_receipt` service run.
  That workflow still requires protected-runtime validation.
- Changed Rust formatting and whitespace checks passed. Private local scratch
  was empty and removed. No remote jobs or files were created.

Logs in `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
d2e09c41e2a2e5c1b7513b75bdce69c49a78f19f514720191d622f87b4cd90d9  cargo-native-custody-tests-r5-20260927.log
f5807e4a80c8e2e7d4dd0a2498479972a81a0a4f7434fddcf2a83592ea4c169f  cargo-native-custody-boundary-tests-r4-20260927.log
54b07451f260ef67e9ee635ecbd5331067c3326adea04e9edd1aed8e28774041  cargo-native-custody-consumer-check-r2-20260927.log
```

## Remaining Integration

The native owner is compiled but is not yet selected by the production wrapper.
Broker profile transport, authority release, the prepared Command/child launch,
parent invocation custody, V5 consumption/finalization, and the generated host
consumer must migrate coherently before selecting the V3 deployment inventory.
The new owner supplies readiness and receipt validation, not that entire switch.
Full bounded live-process capture and the previously unmerged diagnostic and
deployment candidates also remain outstanding.

All M0-M7 acceptance gates remain open; no kernel manifest entry changes status.
In particular, protected vecadd, advanced-kernel matrices, legacy retirement,
target-matched 47/47 hardware qualification, release CI, and identical public
repository heads have not been established by these tests.

Native agent creation failed at the service's thread limit, so this checkpoint
was implemented and reviewed by the primary agent. No Qwen worker was used.
All three SSH aliases (`mi350`, `mi350-2`, `mi300x`) failed DNS resolution. The
initial fetch of origin/main also failed DNS. Publication status must be reported
from the actual subsequent push results, not inferred from this local evidence.

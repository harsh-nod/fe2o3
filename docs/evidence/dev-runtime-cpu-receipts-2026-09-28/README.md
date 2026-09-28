# Runtime CPU Receipt Composition

Development evidence for actual runtime Active/Pending handoffs through the
public backend SPI with genuine KFD CPU-fixture receipts. Not Context/Worker
qualification, native CODE/DATA binding, GPU execution, new formal refinement,
or HIP/HSA behavioral/performance parity.

## Source

Signed source: `7c46c4238881f8c4533ad2f03d91fc8f72b617ef`.
Tree: `9d8aa3998f88cc66a8615c73f174ddd8e2e1ed89`.
Integration parent: `61517a85a68dd1e7dc2590fe249d99c9d16a7ccd`.

## Qualification

| Check | Corrected-source result |
| --- | --- |
| Focused CPU runtime composition | 7 groups passed; 8 terminal publication children and 1 Drop child |
| Full all-feature runtime library suite | 1813 passed, 0 failed, 28 existing ignored |
| Runtime all-feature doctests | 52 passed (8 compile-positive + 44 compile-fail) |
| Strict runtime all-feature/all-target Clippy | Passed |
| Runtime no-default-feature compilation | Passed |
| Workspace formatting and source continuity | Passed |
| Source signature verification | Passed |

`final/qualify.sh` exits zero with `aggregate 0`. No implementation was edited
during either frozen campaign; documentation was drafted while checks ran. The
full passing library run took 84.11 seconds. Focused counts overlap the full suite;
the 28 existing ignored cases were not executed or newly waived. The KFD library
suite, other model suites, solvers and native workloads were not rerun here.

Failures are preserved, not hidden: a preparatory tuple-pattern compile error,
an incorrect test assumption that submit cannot publish immediately, and a Clippy
case-selector lint were corrected. The first frozen campaign passed focused and
static gates but aborted the full library suite with an owner-thread stack
overflow (`aggregate 1`). Compiled DWARF showed the inline optional CPU fixture
added 40,608 bytes to every test backend, including absent fixtures. Boxing only
that field reduced the compiled backend from 104,072 to 63,472 bytes. The entire
second campaign passes without increasing thread stack limits. Raw DWARF output,
review notes and both campaigns are archived.

## Architecture

One private concrete queue/lane adapter delegates ordinary initial/Prepared
submission, ordered submission, polling and recycling. Native calls retain the
same lower APIs, nested callback result types and inline failure receipts.
Submission-result deposit and completion receipt/phase transitions, including
first-Ready timing, remain inside the lower lane callback. Publication timestamping
and confirmation, Pending handoff, retained-resource settlement, profiling and
logical commit remain in the existing runtime callers. The adapter adds no native
heap allocation or dynamic dispatch.

The nondefault `fe2o3-runtime/cpu-runtime-fixtures` feature forwards the lower
fixture feature. The runtime CPU provider and materialization hooks additionally
require `cfg(test)`; this is not an application execution backend. The test-only
provider is boxed to avoid increasing every owner-thread backend stack frame by
the full CPU queue session. Default runtime storage is unchanged.

CPU admission requires read-only synthetic host bindings and an exact
fixture-issued lane roster, excluding native queues, admitted devices and
scripted providers. CPU materialization skips native CODE/DATA construction but
installs the normal `MaterializedBinding` with no scripted outcome. Actual shared
attempt and completion cells hold opaque lower receipts; Pending custody is not
manually settled by the fixture.

CPU cache release rejects live/dirty/resident/native or scripted custody before
mutation. Only clean logical descriptor metadata is discarded, with no fabricated
native DATA-detachment receipt. A cached lane currently requires both lower
fixture lanes clean. Public resource releases precede checked CPU shutdown;
shutdown validates before removing the provider and handles. Drop checks lower
custody before the existing scripted-disarm shortcut.

## Behavioral Coverage

Seven focused groups cover:

- Public Pending acceptance via a genuinely completed event, initial publication,
  exact recipe handoff, unchanged allocation/module/reservation custody and
  exactly-once publication events. Both primary and AUX lanes are exercised.
- Real completion-arena saturation, one dispatch-generation advance per retry,
  retained Prepared custody, zero publication events while blocked, actual arena
  drain and successful publication/recycling.
- Public A/B/C ordered submission, explicit-event retain release once per handoff,
  distinct receipt identities, physical B/C retirement before A, retained logical
  ownership until the frontier completes, and logical A/B/C completion event order.
  A separate variant promotes a still-Published C with the same receipt identity.
- Eight isolated outer-error/unwind cases across initial/ordered publication and
  both lanes. The exact returned receipt remains indexed and poisoned, Pending
  handoff happens once, trailing Pending facts survive, and no target publication
  event or successful result is emitted. Ordered owners are quarantined. Passive
  identity and snapshot comparisons require no post-poison authority.
- Cache refusal for modified descriptors, aggregate dirty state, resident metadata,
  scripted-provider conflict and genuine lower pinned custody, with unchanged
  descriptor/lower snapshots. The resident refusal fixture contains no native DATA.
- An isolated intentional Drop abort when lower custody is live but no runtime
  Active/Pending owner is indexed. Core dumps are disabled for this child.
- Public Write/ReadWrite rejection before handle issuance, retained-resource
  changes or lower effects.

Successful scenarios check clean lower owners, use public resource releases,
perform repeated shutdown and ordinary Drop without disarming it. AUX tests frame
the primary runtime owner and lower lane. Terminal children retain poisoned
custody until process exit; they do not claim recovery or clean teardown. Explicit
child markers prevent zero-test subprocesses from passing unnoticed.

## Remaining Work

Prepared cancellation, native binding/rebinding, writes/readback, currentness
faults, populated-arena ordered retry and runtime pin-recycle retry remain outside
this packet. Outer failures after genuine Retryable and the completion
Pending/Ready/Retired outer-fault matrix still need composed runtime coverage.
Lower snapshots omit full dependency-ledger contents and completion-owner phase.
Shared-source caller refinement, retained-resource/Context proofs, protected
Worker/compiler integration, multi-device/distributed qualification,
atomics/collectives and matched HIP/HSA performance remain open.

No Verus, MI300X, native workload, benchmark or remote cleanup was run. A1/A2 and
the accepted Native R125, Admission R118B, Resources R116/V3 checkpoints are not
promoted. Issue #182 remains open.

## Reproduction

The archived `final/qualify.sh` serializes focused tests, the full all-feature
runtime library suite, runtime doctests, strict all-target Clippy, no-default-
feature checking, workspace formatting, source/signature/continuity checks and
an authoritative issue API observation. No implementation edits occur during
each frozen campaign. Test timing is not GPU performance evidence.

The raw campaign is
`/home/harsh/.codex-tmp/fe2o3-runtime-cpu-receipts-20260928-aDRIRy3e`.
The parent logs preserve preparatory failures and the first unsuccessful frozen
campaign; `final/` is the corrected-source qualification. Verify the immutable
archive with `receipts.tar.xz.sha256`. Documentation publication observations are
kept separately, not appended retroactively to the archive.

Archive SHA-256:
`7d96cd07904fe9f0a546448e464e34d0118f211ec46c27de34768ef033313cb9`.

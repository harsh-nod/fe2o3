# Local R94 Primary Matrix Evidence

Source baseline: signed planning commit
`56ba2556c9572d30f65faab3e579d95fa3fb7c3e`, above accepted R93
`c35bb542f338d41a8247850e24166282c22d4926`, on
`codex/r65-runtime-drain-versions`. This packet extends the existing
[primary constructor](../../runtime-primary-queue-construction-custody-v1.md);
it does not add another constructor or a compatibility backend.

## Status

Locally accepted for the named NATIVE-2A.3 CPU/local-helper matrix. All seventeen
frozen-source gates and eleven auxiliary checks pass. GNU and musl each pass
2,508 runtime tests with five ignored across 48 harnesses; their KFD harnesses
each pass 928 tests. The primary filter passes 25 tests, including the seventeen
integrated functions below. Host tests pass 258 on GNU (four ignored) and 141 on
musl. Seven macro fixtures and 151 Python tooling tests also pass.

Five corrected compiled negative mutations reject at the intended named tests,
and all 5,645 non-documentation source identities match the formatted guard
baseline, final gate input and restored worktree. No new solver, live KFD or
performance result is claimed.

## Integrated Matrix

Twelve new test functions extend the original five. The seventeen functions run
548 cases through the production-used sequence with the original memory session,
Host/Device accounts, data descriptors, actual queue resource authority,
authenticated foundation and native queue engine. These counts are loop cells,
not 548 distinct test functions or an exhaustive state-space proof.

| Test family | Cases | Principal oracle |
| --- | ---: | --- |
| Success | 6 | Three ring backings and owned/external runtime; exact final ownership and observations. |
| Borrowed boundary error/panic | 78 | Original root, GTT/platform partition and terminal threshold. |
| Real fake-native failures | 22 | Original R88 tokens, native error/panic and unchanged account custody. |
| CREATE uncertainty | 5 | Published ownership retained; no unpublished cleanup after publication. |
| Currentness error/panic | 16 | Every constructor observation, including both doorbell checks. |
| Preparation stage error/panic | 62 | Original descriptors, code/identity metadata prefix and completed owner. |
| Preparation native allocation failures | 60 | Exact pending stage or installed output, reservation/ALLOC/mapping and no cleanup. |
| Preparation seal/map/materialization failures | 82 | Exact precursor typestate, native map prefix, error/panic and borrowed storage. |
| Preparation projection failures | 56 | Actual allocation/map adapters, exact input/output and uncommitted model. |
| Generic returned preparation | 8 | Actual charged token, borrowed text, non-Send owner and one callback/drop. |
| External runtime handoff | 16 | Caller slots retained through validation/currentness, moved before gate-arm. |
| Missing/foreign/wrong-role external owners | 6 | Exact original caller slots and anchors, no event admission. |
| Late CREATE output/ID/dependency recovery | 8 | Actual created engine and unassembled owners; exact error classification. |
| Platform success/shadow settlement | 5 | Unique roles/IDs, event/context/CREATE/PID binding and shadow phase. |
| Local Linux helper composition | 16 | Actual mmap/header/protection/cleanup and local gate, including pre/post-publication panic. |
| Event/doorbell observation panic | 4 | Dependency or returned doorbell already rooted before assembly/observation updates. |
| Constructor allocation/map projection | 98 | Every resource role, all ring backings, actual terminal token and model snapshot. |

Allocation projection failure can leave an installed output without a consumed-
input marker. The partition oracle accounts for that actual output separately;
map markers still identify exactly the R88-retained token. Projection snapshots
are taken immediately before the selected adapter call. These fixtures have no
released-record checkpoint work. R88's allocation progress fields are not native
allocation counters: `attempted == false` there does not mean no allocation ran.
Certified revision exhaustion is not synthesized in this uncertified fixture.

## Production Changes

Private default environment methods still forward CREATE-output and native-ID
recovery to the original engine, and dependency construction to the original
owner. The constructor retains outputs before ID recovery and computes the
dependency owner before extracting any assembly fields.

Dependency allocation failure now uses the existing error mapper instead of
being mislabeled as an invalid session occurrence. Both remain terminal under
the same construction stage, with the actual created queue retained.

Checked creation-gate finalization is factored into one private function used
by the production borrowed arm and the test's Arc-owned local arm. Its predicates,
poisoning and success transition are unchanged. No new public runtime API,
production backend selector or new memory-accounting policy is introduced.

## Local Linux Boundary

The cfg(test) helper reserves its own complete CWSR range with `mmap`, then runs
the existing shadow installation, header initialization/readback, write-access
restoration and payload cleanup. The local reservation is independent of the
fake context BO. A separate test-only read-only protection step exercises real
restoration; it does not establish native BO aliasing or sealing.

The event is a synthetic binding backed by an owned `/dev/null` descriptor.
No KFD event/queue ioctl or doorbell mapping executes. Gate state is an isolated
local mutex, not the process-global gate. The actual returned wrappers live in
the constructor's owners, not in the observation ledger.

Before root disposal, unpublished failures check the actual terminal-release,
active and payload-mapping flags and preserve disposed metadata. Published cases
retain the payload. Test-only disposal checks payload release, whole-reservation
unmap and event/file lifetime; failed unmap aborts. No destroyed-event authority
is manufactured to dispose the simulated CREATE. Both runtime modes cover
prepublication and postpublication panic with the exact original panic payload.

## Mutation Oracles

Each mutation compiles and fails the selected test, rather than failing to build:

1. Move external caller-slot extraction before closing currentness.
2. Delay output retention until after native queue-ID recovery.
3. Substitute the PID passed to doorbell mapping.
4. Substitute the assembled event ID.
5. Skip actual terminal unpublished-payload cleanup.

The complete 5,645-file non-documentation source inventory is restored after
each campaign. The accepted campaign is repeated after the source-guard repair
and formatting; the original campaign remains preliminary evidence against its
own snapshot. Retained patches, exact test-selection commands, named failed
tests, source inventories and assertion markers bind each rejection to its
intended change. No mutation remains in the runtime source.

Raw records remain byte-for-byte copies. The local `.gitattributes` follows the
existing evidence-archive convention to prevent text conversion and whitespace
normalization of transcripts and patches; source checks remain unchanged.

## Attempts And Review

Earlier attempts remain separate: the first preparation integration failed on
an ambiguous test import; the second passed nine tests with a helper-visibility
warning subsequently corrected. The restored platform fixture passed thirteen
tests. The first combined local/projection build failed because a test module
was private; its corrected suite passed seventeen tests. Clippy then rejected
the generic runner's return type; a named alias corrected it and preflight
passed. None of those earlier snapshots substitutes for final-source acceptance.

The first full-source attempt, `r94-final`, stopped at GNU tests with 2,507
passed, one failed and five ignored across 48 harnesses. Its KFD harness passed
927 tests and failed the dependency-owner source-shape assertion, which still
searched the old constructor location. The corrected guard includes the
environment method, preserves exactly one real constructor and checks the
primary call and default forwarding body. Its focused test passed; a formatting
check then required assertion wrapping, and the formatted guard passed again.
Fresh corrected mutations and exact source restoration passed before the
`r94-corrected` full-source attempt. Both campaigns and the failed full-source
attempt are retained separately.

Three read-only agents reviewed native/local cleanup, platform/completion
identity and preparation/projection custody. Primary integrated their findings:
resolved-code metadata checks, exact map typestate, generic platform partition,
event binding/observation panic, actual pre-Drop payload state and local unwind
coverage. Follow-up reviews found no remaining blocker within the named matrix.

## Open Boundaries

Auxiliary constructor custody is NATIVE-2B; recycled replacement and insertion
remain NATIVE-2C. Callback-internal or Linux-installer prefixes that fail before
returning ownership are not recovered by an outer root. Acquisition before the
memory session is returned also remains outside this envelope.

Concurrent bootstrap is still unsupported. No generated adoption/issue/typed
completion, Context version journal, aggregate backing/control/code budget or
public async API is enabled by these tests. Process-lifetime retention is not
bounded aggregate accounting.

No Verus solver, SSH, GPU workload or remote staging was started. Model/proof,
resource-accounting, completion and manifest/lockfile inputs remain unchanged
from R73 `fb1e27e66cee27bb11f0b7c08f2d5994c5d168da`; the negative inventory check
is not a new solver result or executable-refinement proof. Live KFD qualification,
protected compiler integration, matched HIP/HSA performance and full A1/A2/parity
remain open. No shared-machine resources were created or removed.

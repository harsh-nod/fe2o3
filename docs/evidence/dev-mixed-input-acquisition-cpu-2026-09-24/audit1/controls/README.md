# Atomic Mixed Launch-Input Acquisition

Status: all fifteen CPU qualification stages pass, including 73 doctests,
formatting, strict Clippy and unchanged source/tool brackets. No native execution,
authenticated formal correspondence or performance acceptance is established by
this packet.

## Implementation

One journal operation validates both stable and pending-producer rosters before
either commit. It checks the Submission consumer, both output shapes and shared
capacity, including empty-side cases. Failure returns the original journal and
both output arrays unchanged. Success uses the existing allocation-free commit
helpers without rescanning admitted requests. Empty sides do not inspect their
unused arena's incarnation counter.

Context retains both original input roots before acquisition, fills both
preallocated reference vectors, then publishes both markers before backend
entry. Writerless consumers and each empty-side combination remain supported.
An acquire error retains both unmarked roots without committing a lease. A
post-acquire panic retains all acquired leases, keeps both markers unpublished,
preserves the original panic payload and seals the Context. Neither case enters
the backend. This is fail-stop custody, not panic rollback.

The production delta is confined to mixed acquisition and its launch adapter.
Ordinary single-roster acquisition and directed-peer acquisition are unchanged.
The fault-injection field and controls exist only under `cfg(test)`.

## CPU Qualification

`qualify.py` requires fifteen bounded commands: compiler/tool identification,
formatting, strict all-feature/all-target Clippy, GNU and musl default-feature
checks, focused and full runtime tests, full model tests, GNU doctests and final
tool identification. Source and runner maps bracket the complete campaign.

The accepted rosters are 1,403 runtime passes plus 22 hardware-only
ignores and 1,028 model passes plus 18 existing manual performance/scale ignores
on each target. The 38 focused passes and two hardware ignores are a subset of
the runtime roster. The verifier requires every inherited named result plus
exactly three runtime and seven model additions. It checks three doctest group
summaries totaling 73 passes, not an exact named doctest roster. All 3,965 source
inputs and the exact thirteen-path delta are checked. Earlier cache contents
are reused; this is not a cold or hermetic build attestation.

Replay from this source checkout:

```text
python3 -I -B docs/evidence/dev-mixed-input-acquisition-cpu-2026-09-24/qualify.py --output /home/harsh/.codex-tmp/fe2o3-r61-execution/docs/evidence/dev-mixed-input-acquisition-cpu-2026-09-24/raw/cpu1 --verify
```

The replay intentionally binds the current checkout and recorded absolute
command paths. Later source changes require returning to this published source;
the verifier does not reinterpret historical results against a changed tree.
Eight verifier-test groups exercise command controls, exact source scope,
missing/aliased artifacts, baseline authentication and both named test rosters.

## Development Proof And Attempts

The new Verus execution relation uses the same mixed-acquisition macro as Rust.
It specifies exact error precedence, error-state preservation, the two commit
relations and shared-budget preservation under storage-domain preconditions.
Three connecting lemmas preserve producer readiness and budget headroom.

The root includes inherited paired proofs but adds no independent logical
mixed-acquisition correspondence or constructor-origin mixed witness. Context
maps, original binding coverage, writer Begin plus readers as one transaction,
panic/unwind, async carriage and producer-first reconciliation remain outside
this candidate. No result here establishes machine-code refinement.

Development logs are diagnostic, not authenticated proof receipts:

- Seven model tests passed.
- The initial runtime test compile failed because `unwrap_err` required a
  `Debug` implementation for the success type. The corrected focused run passed
  38 tests with two hardware-only ignores.
- The first proof command rejected an invalid scoped CLI combination. The
  second reported one proof error; explicit unit-result equality resolved it.
- The scoped actual-method run then reported one verified obligation and no
  errors. That run alone does not verify the three new helper lemma bodies.
- The first whole-root attempt ended with SIGTERM, exit 143 and empty output.
  It is not a proof result.
- The fresh whole-root run completed with exit zero, 1,244 verified obligations,
  zero errors and `is-verifying-entire-crate: true`. This covers the three new
  helper bodies and actual mixed method as well as the inherited obligations;
  the counts overlap older proof results and must not be summed as independent
  coverage. The command used the installed Verus
  `0.2026.08.09.92f466f`, four threads, default SMT limits, `--no-cheating`,
  JSON output and a 1,200-second outer bound. Its logs are development evidence,
  not an authenticated proof campaign with tool/source brackets and mutations.

## Retention And Limits

`finalize.py` records CPU replay, eight verifier-test groups, five inherited
cleanup-test groups and six new cleanup-gate groups. Optional cleanup requires
the exact scratch/command rosters, terminal process groups, no same-UID process
referencing the private tree through arguments, cwd, descriptors or mappings,
and unchanged complete owned-tree identities. It copies all development logs
before removing only the task-owned scratch directory and records independent
ordinary-path and symlink absence in one command. Process and filesystem scans
are observations, not an exclusion lock against a hostile concurrent creator.
The preexisting repository target and unrelated owner-inspection evidence are
outside its deletion scope. No remote workspace is used.

The previous MI300X producer-chain result qualified earlier signed source, not
this implementation. Fresh native qualification, independent logical mixed
correspondence and mutations, completion reconciliation, protected generated
graphs, aggregate accounting and matched performance remain required. A1/A2,
the accepted lane checkpoints and full HIP/HSA parity remain open.

The next proof extension should compose the existing logical stable and
producer relations, not duplicate their batch loops. Mixed error precedence
uses the raw logical stable decision, not the producer stable-wrapper decision.
The intermediate stable commit must preserve pending validation and journal
contents while reducing combined headroom by the exact stable count. Existing
paired transitions then establish final representation, exact outputs and the
logical invariant. Constructor-origin mixed witnesses and independent negative
controls remain separate gates; the historical authenticated checker must not
be weakened to accept changed source.

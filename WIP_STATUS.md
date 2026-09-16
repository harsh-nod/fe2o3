# Issue 271 Source-Preservation Checkpoint

Reviewable WIP, not production activation or tutorial qualification. Do not
merge this historical tree wholesale over current main. M5/M7 remain active;
no acceptance milestone is newly complete. M8 remains incomplete.

## Exact Source

- Host: XSJHARMENON01.
- Checkout: /home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913.
- Checkout HEAD: 10b190b8267c0c4011b4ef6bbb52680a3c44b391.
- Source: 5,071 files, manifest SHA256
  da475ae403553e4ed8670603b8d78a452c24ed8a17462fde9b21e87f1805548b.
- Previous published WIP: e850450f837526650a6481ed260b03af0e56e219.

The original checkout HEAD/index remain unchanged. This status is outside the
source manifest; snapshot construction verifies every source Git blob. Seven
compiler paths change from the previous WIP, plus this status file.

## Implemented

An independently checked source-to-original-neutral-IR relation now covers a
closed getter/Option/Store subset. It checks actual admitted source statements,
operands, types, complete control/operation coverage and exact original IR.
Importer replay, a matching digest or matching counts do not replace these
rules. Source Store lookup uses a paid binary search over a privately checked
ordered, unique roster. Retained queries bind to the same live ledger/floor;
callback errors, panics and resource failures retain checked cleanup behavior.

An additive private all-root consumer composes that relation with original
Expression/R1 checks, N/B target binding, checked B/O, actual output D/P/R2,
and complete final formal-memory analyses of the same optimized output. It
preserves the original reference object and existing nonempty-reference gates.
Empty references remain genuine functional None and aggregate absence. The
consumer is not connected to production custody, lineage, artifacts or launch.

The admitted subset is direct Unit roots with authenticated Global U32 slices,
Global-X/get_mut, the exact Option discriminator/control recipe, and U32
constant/formal/copy Stores. It rejects uncovered operations, entry Defined-call
wrappers, arbitrary helpers, loops, merges and unsupported arithmetic. It is
not a universal Rust semantics proof or a formally verified whole compiler.

## Exact-Source Qualification

All final-source runs passed their before/after source and helper checks:

- 411 lowerer library tests, zero failures or ignores, 12.99 seconds.
- 890 backend library tests, zero failures or ignores, 164.34 seconds.
- 20 extractor binary tests, zero failures or ignores, 0.04 seconds.
- Two selected compile-fail documentation tests, 0.13 seconds. The observed
  errors are the intended non-Clone bound and escaping proof-borrow lifetime.

That is 1,321 ordinary tests plus two compile-fail tests, not the entire
workspace or all tutorial kernels. Retained diagnostics are:

- v257-clean-v349-source-preservation-full-libraries.xXXe1xQE
- v257-clean-v349-source-preservation-extractor.gcZgNqHY
- v257-clean-v349-source-preservation-doc-tests.rc4o22F8

The seven new lowerer tests cover genuine admission/capture, both discriminator
modes and target profiles, hostile original-IR changes and resource boundaries.
Seven new backend tests exercise the actual conjunction for one/two roots,
constant/formal values, reference gates, foreign owners/ledgers and callback
error/panic/reentry. These use admitted semantic factories, not fresh rustc
collection. Component resource thresholds are not whole-engine budget claims.

Earlier failures remain recorded: an unused signed type in the unsigned fixture
was removed only for that variant; an old source audit accidentally included the
new consumer in its source-only text range. The latter now has checked function
delimiters. Every original audit assertion and production gate remains intact.

## Conditional Premises And Remaining Work

Standard slice validity, exclusivity, lifetime, alignment and extent premises
remain conditional. The current rule also retains an inactive-address
representability premise because the importer emits GEP unconditionally.
That stronger premise does NOT follow from valid safe Rust get_mut on a short
or empty slice. It must not become an extra user obligation merely to activate
the compiler. A reviewed importer/operational-semantics correction and matching
source/output/formal rules are required before production activation.

Entry-wrapper semantics, broader source-rule coverage, the exact runtime ABI
join, optional user-reference refinement, production custody/lineage wiring and
legacy retirement remain open. Fresh actual collection of this new conjunction
has not run. The preceding source 4781066c passed seven actual-collector tests
and a direct transcript, but that checked original R1 plus output D/P/R2 and did
not execute this new source relation. Its result is not transferred here.

Both main branches separately contain ec2b88cfb050cb954a426f9968b8a73e1688ad37,
whose 297 lowerer and 648 backend tests and repository publication checks passed.
Its formal-contract CI passed; generic CI was still running at this checkpoint.
Actual main attention compilation still fails: pipelined attention has the
multiple-definition refusal; scalar attention has a missing private ranked
effect. A separately reviewed private-write metadata fix is not included here.

Full tutorial production compilation, deterministic simulator comparison,
target-matched hardware and website qualification remain open. Approved Verus
runtime qualification remains deferred, not passed. No new GPU observation or
remote shared-machine work was performed for this checkpoint. The CPU and
separate issue 272 WIP branches are unchanged.

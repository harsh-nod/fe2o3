# Fresh promoted composition: source, normal outputs and static native checks

Qualification date: 2026-09-24. This is a bounded continuation of the
[source-promotion workflow](ordered-composition-source-promotion-v1.md), not
completion of issues #280–#282.

## What completed

The actual extractor published three new ordinary Rust candidates: a copy, a
preserving typed edit, and an intentional MoveInput2 edit. Each candidate was
re-admitted through a fresh ordinary frontend, checked executable graph, CPU
observation, normal LLVM output and inert handoff. No generated candidate or
LLVM artifact was patched to make this work.

The seed adds an explicit source max_grid=[2,1,1] before compilation/publication;
the checked-in dynamic-launch fixture is unchanged. The original plus three
candidate graphs each ran 32 independent CPU cases (128 total), with four
scalar triples, lengths 0/13/64/129, grids 64/128, an eight-byte output view
offset, canaries and initialization bits. The preserving expression is
(a xor b) and c; the intentional edit returns c.

The parent completed 23 workload children: 13 actual extractor invocations and
10 fresh normal frontend sessions. It retained three publications, six exact
source refusals, 12 candidate descriptor-extension mutations plus three
cross-candidate prefix refusals. Copy and preserve legitimately have equal
source bytes but distinct fresh owners. Actual canonical V17, canonical LLVM,
descriptor V1, worker LLVM and handoff V2 files are retained for all four
checked graphs; candidate LLVM/handoff outputs join their own fresh owner.

An independent read-only review inspected 665 retained files, including the
exact 459-file / 350,983,677-byte dependency roster, four genuine package
metadata identities, all 13 CLI terminal records and ten callback records.
It independently parsed handoff framing and reconstructed descriptor-byte LLVM
extensions. That audit is not another numerical or native execution.

## Static native continuation

A separate promoted-source observer uses the existing ordinary LLVM/LLD worker
and unchanged strict machine-effect/trace analyzer. It accepts only the three
actual candidates at O0/O3, never the older seven-profile report schema.

The declared roles are scratch v8, output v9, and inputs v10/v11/v12.
The independent programs are xor/and descriptors [133,315] or MoveInput2 [40].
The native checks passed **6/6** cases, **42 LLVM**, **108 metadata** and
**66 decoded-machine** mutation controls. Observed architectural capacity must
cover v12; minimum 13 is not an exact register footprint or a no-AGPR claim.
Compiler-added resources, hidden queue offset 232 and dynamic-stack descriptor,
metadata and absolute-symbol consistency remain mandatory.

The shared test-only observer code was also rebuilt in its original default
profile. Its fresh compatibility matrix passed **14/14**, with 80 LLVM,
252 metadata and 154 decoded controls. All fourteen HSACOs are byte-identical
to the earlier R7 artifacts. This is new compatibility evidence; historical
failed matrices remain untouched.

Before these matrices, strict C++ builds passed 12 promoted/legacy profile
groups and the existing 40 resource controls; 46 separate Node relation
controls also passed. No production analyzer opcode or effect predicate was
relaxed.

## Retained qualification records

| Gate | Receipt SHA-256 | Report SHA-256 |
| --- | --- | --- |
| Fresh compiler CPU/build R3 | `10dd5a5ceee500c4610d6eddc9216c92d730cd57503601d882e9e91bc27234f0` | — |
| Fresh promoted normal source R1 | `b2ecfb606c2a6631974079c8f4d25ae192bede131be8065abc042ca98e2a964a` | `2b91e1b35768f0e0de4a3b7e37bc5e29cbf82469ee1f3f2fa548751ad4b69d46` |
| Promoted/legacy observer build R1 | `e2d93f8a1fd70bb77e03db0255451263c75b55774aff97b55a47af7c00a1d059` | — |
| Promoted native matrix R1 | `81b6aace043ce9de9028db79e596eb807879254cebd90142fd4cf9116dd0c356` | `0843395619b697cef25747ea6608418396bde58fb967fc85f5e852aaab97f1bf` |
| Original-profile compatibility R8 | `d37cbbec50d9ededea6a7b3e4d3706ece42a45febf3327ee1c4581acae9beb2b` | `a462cf4fa58bebf0c778d2f837f309ba548cefc3961a0f2b05ef9c39e4cded68` |

The normal source report is 196,545 bytes. Its source census is
`2510b9a1917ac61ea475c52d287181df0748e5199a8b5b6f28e4a91d09c7393d`
(7,822 files / 113,901,858 bytes). The native reports are respectively
741,031 and 1,918,269 bytes; their shared source census is
`2d711f8b4f1591faaaac7c7526d0d4bfd55698f1095af2ee924c7c413dc19796`
(7,828 files / 113,926,411 bytes). These are their actual gate snapshots, not a
claim that later documentation or unrelated fixture-lock corrections were built.

The independent source audit has SHA-256
`576e52e0fe3574713c6a5cc106f02ca7100820ca0f84948648af2f1c5517694a`
(12,381 bytes). Root additionally joined child exits, stdout and saved reports,
all actual artifact hashes and before/after source/tool/input observations.
The pinned LLVM SDK passed complete pre/post file verification for each matrix.

## Whole-backend regression and fixture lock closure

The unchanged offline locked all-targets command for `rustc-codegen-fe2o3`
passed **2,200 tests in 22 result groups**, with **247 tests still ignored** by
their existing declarations. The ignored source/native cases are not counted as
executed by this command. Its receipt is 44,446 bytes, SHA-256
`941feb539e479dc5345c845cba6b458f69edb713895d25f2fd601e501d872a1d`;
the exact source census was 7,830 files / 113,941,621 bytes,
`db563ae5ddb25012c7110c6320f5cf691e4e5b29bd93557ad325f6bd49bcc04a`.

Earlier all-targets attempts stopped on stale standalone fixture locks, first
cross-crate kernel-a and later g2-semantics. Their failed receipts are retained.
Thirteen fixture locks now include the missing current local dependency edges;
five small locks also add the exact workspace-pinned kernel-ir dependency
closure. Every preexisting registry version/checksum/block remains unchanged.
The repaired cross-crate and g2 integration tests both passed without dropping
`--offline` or `--locked`. This is not a dependency upgrade or a waiver of tests.

After that gate, only this qualification text and two redundant trailing blank
lines in the test-only C++ observer project changed. The original native reports
retain their actual earlier source snapshots; no report is retrospectively
assigned a later source census.

## What remains unproved

Static authored intervals do not establish physical helper argument/result
transport, root guard/address/store functional equivalence, complete dynamic
execution order or GPU behavior. O0 retained calls and O3 inlining need distinct
value-transport reasoning; an interval match is not a calling-convention proof.

The ordinary checked owner retains the additional unresolved **512-byte
writable-output** requirement. Runtime pointer/extent/permission conditions
have not been discharged. File digests and JSON records cannot reconstruct
compiler/source custody, admit an artifact, or permit a launch. Observers are
bounded diagnostic programs, not alternate compilation or native admission
routes. Direct-child/process-group cleanup observations do not prove an entire
process-family boundary.

The one-stop debugger producer and tiled-compute source/materialization workflow
remain separate work. Accepted broad exits remain **M1/V1/V2/U1/U2/U3 (6/18)**.

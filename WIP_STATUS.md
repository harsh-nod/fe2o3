# Issue 271 Guarded Getter Integration Checkpoint

Reviewable WIP with five known failing formal-consumer tests, not production
activation or tutorial qualification. Do not merge this historical tree wholesale
over current main. M5/M7 remain active; no acceptance milestone is newly complete.
M8 remains incomplete.

## Exact Source

- Host: XSJHARMENON01.
- Checkout: /home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913.
- Checkout HEAD: 10b190b8267c0c4011b4ef6bbb52680a3c44b391.
- Source: 5,071 files, manifest SHA256
  f9d0117a61d3e8e18133dadd19d14713bf15e1b2443d2061bf45ed487df2d408.
- Previous published WIP: fde5bd5ec2749ce9f26f76b626c808eebab3a0d0.

Snapshot construction checks every source Git blob and preserves the source
checkout HEAD/index. This status file is outside the source manifest. Ten source
paths change from the previous WIP, plus this status.

## Implemented

Checked slice access lowering now selects zero for the inactive index before
forming its element pointer. The source-to-original-IR checker requires that
exact recipe before removing the old inactive-address representability premise.
It does not make arbitrary Select operations affine or erase dereference bounds.

The output-control, physical-address and functional-address relations retain an
exact private selected-offset description. Each actual Some-branch Store joins
its own source/output occurrence, predicate, raw index and selected definition.
The physical relation describes the actual selected offset, not the raw index.
All components use the existing resource ledger and retained live-storage floor.

New backend tests compose the genuine source/original relation, Expression/R1,
control and physical/functional address relations for both gfx942 and gfx950,
both discriminator encodings, and one or two distinct Stores. Independently
verified hostile output mutations are rejected at the real checked-transition
boundary. Those refusals are not mislabeled as address-relation coverage.

The existing final formal-memory Complete gate is unchanged. Its five genuine
positive tests remain failing, not ignored, inverted or replaced with weaker
component tests. Source-preservation and address-relation success alone does
not establish end-to-end memory safety or compiler correctness.

## Exact Test Results

The latest exact-source focused run passed 44 tests and failed five, with zero
ignored and 846 filtered out. The new composition tests and updated importer/
physical-shape oracles pass. All five failures occur because the unchanged formal
engine rejects the selected index as UnsupportedIndexExpression before the final
consumer callback. Logs:
v257-clean-v356-getmut-backend-relations-imports.vCdx2egG.
Source and helper guards passed. Test execution took 25.90 seconds after a
1m37s build.

On preceding composed source
37933a05e508df0650442cc907690bf06f64d38e14dfdee342b142ad88b4c91b,
425 lowerer tests passed, with zero failures or ignores. The full backend run on
that source passed 886 and failed seven: two stale shape oracles subsequently
corrected here, plus the same five formal-consumer failures. Logs:
v257-clean-v356-getmut-composed-lowerer.A6wjgMwS and
v257-clean-v356-getmut-composed-backend.kGpZSGXu. All input guards passed.

The first test-successor build failed on two missing test imports; ROOT added
Type and ValueId. That failed compile remains recorded at
v257-clean-v356-getmut-backend-relations.ocMwfcic. The full lowerer/backend suites
and binary targets have not been rerun on this final source. No tutorial, actual
collector, hardware, site or Verus qualification is claimed.

## Remaining Work

The formal engine needs use-specific guarded access domains. It must prove the
selected offset equals the raw index only for a matching active access, retain
the domain [0, launch_extent) intersected with index < slice_length, and handle
empty/short slices without inventing a launch-sized minimum slice obligation.
Bounds, every alias/conflict pair, resource limits and final-output subject
binding remain mandatory. Existing legacy receipt formats must refuse evidence
they cannot represent. That implementation is being developed separately and
is not included in this snapshot.

Standard Rust slice validity, exclusivity, lifetime, alignment and extent
premises remain conditional. Defined entry wrappers, broader source rules,
normal managed metadata custody, runtime/target binding, optional reference
refinement, production activation and legacy retirement remain open.

Both public main branches were independently read at
17ad92bea81ab7f6f60e74b1b97076c29007451b. Their independently tested bounds fixes
and formatting follow-up are not folded into this historical WIP tree. The
separate initializer WIP remains based on
8a7b98fd73de314e2f0208e82e449852e680e5bd, with further local-frame work under test
in its own checkout. Neither its helper admission failure nor general nonliteral
initializer semantics is solved by this snapshot.

M8 all-tutorial compilation, simulation, target-matched hardware and website
qualification remain open. Approved Verus runtime qualification is deferred,
not passed. No SSH/GPU work or shared-machine cleanup occurred here. Separate
issue 272 WIP is unchanged by this checkpoint.

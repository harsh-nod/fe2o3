# Producer Input Composition Qualification

Development evidence, not release qualification or HIP/HSA parity.

Signed candidate: `6b9e5d87c4386692f7c213a95dd786ccdf42cb45`.
All 102 campaign stages passed: 38 validator, 22 fold and 21 unfiltered
composition mutations; nine full proofs (42/13/64 obligations per family,
each before, relocated and after); six release checks; five source-control
stages; and the signed-commit check. Obligations are not independent runtime
properties. The original recorder closed all 102 owned process groups.
Agent and root independent readbacks agreed exactly.

`campaign.tar.xz` contains the entire immutable 1,218-file accepted campaign,
its original stage streams, the independent readback capture, root readback
receipt, and reviewed qualification sources. Identical payloads use backwards
in-archive hardlinks; no symlinks, external links, nested historical archives
or copied worktrees are used. `archive-index.json` describes every member and
its original local source. The packaging controller verifies all decoded
members and archive links without extracting them or executing archived code.

`signed-candidate.bundle` contains BOTH unpublished signed side commits,
`10013c06a14e876fde141c769b5baf8bc42b3203` and
`6b9e5d87c4386692f7c213a95dd786ccdf42cb45`, over public prerequisite
`ee849c0fc5bf6d7f7bc21e4044b2a152dbfab1f3`. Import it into a repository already
containing that public ancestor. The bundle is not a standalone repository.

## Replay Boundary

This packet is NOT self-contained full replay. The original tools (including
the pinned Verus release), live original and retained CPU ELF paths, complete
historical discovery/calibration directories, and fixed local source checkout
paths are external prerequisites. Their exact identities and hashes are in
`external-replay-prerequisites.json` and the preserved campaign preparation.
Recreating missing paths or obtaining those historical inputs is separate work.
The archived auditor intentionally fails closed when those inputs are absent.
The publication controller packages bytes; it does not rerun Verus, a CPU test,
Clippy, a GPU test, or the already completed independent auditor.

Historical raw buffers explicitly pinned by the campaign remain included.
Entire earlier campaigns are not recursively embedded. The interrupted original
33-stage calibration remains incomplete; its rejected/observational captures
and the accepted 15-stage observational continuation are never promoted to
qualified kills. All 81 qualified negatives are fresh signed-campaign runs.

## Scope

The actual shared validator and fold bodies are conditionally composed through
adapter contracts. Concrete journal forwarding, live-allocation validation,
fresh credit-lock observations, Arc identity and shared interior state are not
established by this theorem. Compiler/ISA refinement, allocation, unwinding,
hardware behavior, performance and HIP/HSA parity remain outside its scope.
The unchanged earlier CPU ELF is retained historical evidence, not a fresh
CPU run or qualification of a later source integration. Dynamic-loader
libraries are not hermetically attested.

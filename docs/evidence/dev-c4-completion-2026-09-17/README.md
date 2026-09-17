# Private C4 Completion Integration

Development only. R125 Native CPU/test, R118B C1/C2/C3 and R116/V3 remain accepted.
This does not close C4, A1/A2, #182, protected Worker integration or HIP/HSA parity.
See [the completion contract](../../runtime-generated-completion-v1.md).

## Source Cohort

The source base is `13c5e5b8126dd461c5d7eb8972613d3ff6f4019b`.
The prerequisite patch is the separately frozen thirteen-file native initialized
readback correction, SHA-256
`fd3f82ee6330210ec91157acfdbd9815ef913e1e0378fcbef7337a850be14c3b`.
`integration.patch` contains only the completion changes above that prerequisite;
`source.patch` contains the entire combined delta, with thirty-eight source and
unsafe-inventory files individually hashed. The source patch, not an implied
intermediate commit or a milestone label, defines this development cohort.

CPU qualification is complete. Fresh frozen-source GNU runs passed 908 runtime
tests with seventeen ignored and 262 host tests with four ignored. Source checks
and executable hashes match before and after both complete test rosters. Fresh
frozen-source strict host/runtime Clippy, workspace formatting, runtime
no-default-features and unsafe-source policy checks pass (the latter has five
passes and one maintenance ignore). Host doctests pass both harnesses, two and
fifteen tests, and runtime doctests pass all thirty-six, including the three
completion compile-fail examples. Source checks match before and after these
gates. The separately scoped `musl-no-hip` rerun also passes 908 runtime tests
with seventeen ignored and 262 host tests with four ignored. Its complete
rosters match GNU exactly, and source and executable hashes match before and
after execution. The final source/command/binary/roster audit is recorded in
`qualification-audit.*`, outside `raw/` so it does not observe itself as an
unfinished command. `SHA256SUMS` seals the complete archive. Integration remains
separate from this CPU qualification; no C4 milestone acceptance is implied.
No native probe is claimed for this integrated Context/carrier path.

The first musl build failed before tests: all host features enabled the optional
legacy HIP C adapter, but this machine has no `x86_64-linux-musl-gcc`. Its failed
`musl-build` receipt is retained. The separately named `musl-no-hip` qualification
uses the existing `FE2O3_HIP_SYS_DISABLE=1` build switch, with the same Cargo
features and frozen Rust source. This disables native HIP discovery and linkage;
it does not disable direct KFD or constitute native HIP qualification. Its
completed harnesses qualify only this separately scoped CPU/direct-KFD cohort;
the initial unrestricted musl build remains failed.

## Reviewed Boundary

The original host destinations, decoder, result gate and completion cell remain
coupled to the original prepared carrier. The same source currentness scope
encloses full readback, read-only validation and native retirement/release.
A private-field, non-Clone receipt binds exact logical/backend submission,
stream and hold and is minted only after the closing check. Context cleanup
consumes that receipt before the async driver consumes the original decoder.
Decoder errors/unwinds cannot commit the gate; Stop never decodes.

The unsafe inventory has exactly three reviewed additions: the correspondence
trait, its private host implementation, and an inert adversarial omission-test
implementation. The test implementation never constructs a source/view, Worker
authority or native device. It only checks the required-callback guard.

CPU tests cover original allocation identity and complete shape/RO checks,
source opening/closing failures, settlement/decoder errors and unwinds, original
reply delivery and Stop/drain behavior. Direct Context tests prove pre-lending
retention on missing native admission and exact post-native bookkeeping using
an explicitly assumed cfg(test)-only receipt, including wrong-token/phase
rejection, two-invocation isolation, exact credit refund and one physical
completion callback. They do not prove that native execution occurred.

## Open Qualification

Independent read-only review found no production ownership blocker. It identified
two missing direct adversarial callback tests: a carrier suppressing callback
failure, and a carrier returning an error or panicking after obtaining a
settlement token. The current guards appear fail-closed, but these branches are
not claimed as directly covered. Mutable destination vectors remain available
inside the runtime; allocation preservation is a reviewed implementation and
unsafe-contract invariant, not a type-system guarantee.

Protected Worker/carrier/Context/native composition and native injected-failure
coverage remain open. The separately recorded lower native readback probes
cannot substitute for them. Public typed completion (C5), generated graph/drain
integration (C6), production version journals and reuse, aggregate memory,
formal correspondence and matched performance remain separate requirements.

# Tutorial root-lock binding maintenance

This is source-contract metadata maintenance for issue #271, not compiler,
execution, proof, hardware, publication, or curriculum qualification.

The retained root `Cargo.lock` digest was
`bdde29e69c17715f71ffccdcddda7dda26c8b94ac0cf76de1dc5a72763b3da4d`;
the current file hashes to
`38634cd028b62ca5d081e431740757bf52161f83440a459e2550675a4e7f4a8a`.
This maintenance does not modify or regenerate any lockfile.

Exactly nine scalar fields change in
`config/tutorial-kernel-manifest-v1.json`: four root-lock bindings, their
four enclosing contract digests, and one dependent source-bound variant
selection digest. The root-lock consumers are `gfx942-fill-simulation`,
`gfx942-scalar-gemm`, `gfx942-typed-vecadd-source`, and tab 0 of
`cpu-semantic-simulation`.

The existing fixture and source-item digest APIs recompute the four enclosing
contracts. Independently, kernel selection identity includes the lock digest,
package inputs, feature selection, Cargo target, and kernel symbol. Therefore
the fill SIMT variant's retained `selectionSha256` changes from
`08d8004cfacedad3814a45d25648008afb2d63707942e6258278bc837b8cde77`
to `b62beb69b327de595bf84c172a066206128ab5df5fd3d90df6446d08e4d73048`.
Its source path, physical source digest, function offset, and implementation
kernel identity are unchanged. The other source-bound variant, row-affine
sum in CPU semantic tab 6, has no root-lock dependency and remains unchanged.
There is no stored aggregate inventory digest to refresh.

The curriculum snapshot golden changes from
`561f23181c4e5dc86ee7d29d5cf72c94eb6cab6797e78ae4f936778ae0f2e406`
to `1834b497c5aaaa954df2cc9898f01f2c42ed8c0061a43b34cf2d496d932ebc0c`.
Its only changed payload fields are CPU semantic tab 0's lock and contract
digests. The existing snapshot test still requires 56 lessons, 307 code tabs,
and the same executable/conceptual and mixed-variant rosters.

## Observation and limits

The existing `validate_manifest` API accepted the in-memory nine-field
proposal against the current checkout. The intermediate eight-field proposal
correctly failed with a stale variant-selection digest. Both existing
source-bound variant identities were recomputed with the existing bounded
selection-identity API; only the fill binding differed.

This observation is not a test-suite or build result. No test suite, compiler
build, GPU execution, or formal qualification was run for this maintenance
packet. The source-contract inventory remains incomplete: 61 known kernel
identities, 123 known variant obligations, two source-bound variants, zero
complete source-bound SIMT/tile pairs, and no validated runtime census.

All kernel and negative-case rosters, source/site/compiler pins, policies,
qualification records, variant statuses and blockers are unchanged.
Curriculum status remains `pending`; compiler baseline remains
`unqualified-source-contract` with null commit/tree; recovery remains
`dirty-proposal-not-qualified`. No previous receipt is requalified by these
digest updates.

The manifest bytes change from SHA-256
`ecd23deb63e277e366f91c1df5627a19abe6ce224af4ad19ba28fe4e803684d8`
to `cdca5a37551f6c1abc443d0aa0f973c112b77e81d5a29a2f0eb067ae14e784ad`.
After integration, rerun the required-curriculum validator and the tutorial
manifest, occurrence, identity, and compile-matrix harness tests before
recording any fresh qualification.

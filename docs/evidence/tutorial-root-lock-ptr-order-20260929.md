# Tutorial Root-Lock Dependency Refresh

This maintenance records the root `Cargo.lock` change following the separately
published Pliron pointer-ordering dependency update. It does not update any
historical website/compiler evidence pin or claim new compiler qualification.

- Previous root lock: `38634cd028b62ca5d081e431740757bf52161f83440a459e2550675a4e7f4a8a`.
- Current root lock: `25362f248f1dff26a0d15316b80a751c260c8d9d29f86c4720e12b4082fbd598`.
- Previous manifest: `cdca5a37551f6c1abc443d0aa0f973c112b77e81d5a29a2f0eb067ae14e784ad`.
- Updated manifest: `f67617a5f75e0642fefcb0dc27896f8b58de92830291b8f02cd2a0af35b48100`.

The manifest changes exactly nine scalar leaves: three root-lock compiler
fixture digests and their enclosing contracts, the root-lock
`cpu-semantic-simulation` source item and its enclosing contract, and the
dependent `fixture:gfx942-fill-simulation:fill` source-selection digest.
The three fixtures are `gfx942-fill-simulation`, `gfx942-scalar-gemm`, and
`gfx942-typed-vecadd-source`. Other lockfiles and source selections are unchanged.

The fill source-selection digest changes from
`b62beb69b327de595bf84c172a066206128ab5df5fd3d90df6446d08e4d73048` to
`8f8ef3bf099d22af51d94825294a8904628cfa7abc1e345ef9514b4f04fb3f4a`.
The independently bound row-affine source selection remains unchanged.

The two exact snapshot expectations are refreshed through the existing sorted
ASCII JSON representation, without changing assertions or manifest semantics:

- Curriculum: `296e789d7e3a20836ab4054e34cc3bd0d8a04dd30f88b25133d93a625cae7100`.
- Kernel inventory: `791fc6a880cf57c67b0a1b044bb24d19986cf3e7bc5b4fb6b357f72c5ffc5f6c`.

The existing bounded source-contract diagnostic validates the proposal's 50
compiler fixtures. Its intermediate eight-leaf proposal is correctly rejected
until the fill source-selection binding is recomputed. No kernel/source roster,
binding location, status, qualification, baseline, production contract, or
pending obligation changes. This schema diagnostic is not a compiler build,
test-suite, tutorial-corpus, runtime, or formal-proof qualification receipt.

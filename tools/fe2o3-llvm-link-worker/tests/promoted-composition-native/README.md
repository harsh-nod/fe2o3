# Promoted composition static observer

This test-only observer follows actual copy/preserving-edit/MoveInput2 source
publication through fresh ordinary compilation. It does not import arbitrary
assembly, reconstruct a compiler owner, launch a kernel or admit an artifact.

It shares the original observer's worker and checks through the explicit
PromotedV8 profile. LegacyV32 remains the default with unchanged constraints and
minimum register coverage. The strict production effect/trace analyzer is not
relaxed.

The input is a completed `fe2o3-test-composition-promoted-normal-ladder-v1`
report, its exact digest, the pinned repository, one of copy/preserve/edit,
O0 or O3, and a fresh output directory. Source bytes/inode/timestamps,
canonical/normal LLVM, descriptor extension and handoff bytes are rechecked.
The distinct output schema is
`fe2o3-promoted-composition-native-observation-v1`.

The profile uses v8 scratch, v9 output, and v10/v11/v12 inputs. Independent
literal encodings check xor/and or MoveInput2; metadata must cover v12, while
compiler-added register/spill/stack facts remain observable. Exact intervals
do not prove physical helper value transport or whole-kernel equivalence.

Configure with the same pinned LLVM SDK and FE2O3_WORKER_SOURCE as the existing
ordered-composition-native test project. Targets:

- `promoted-composition-native-observer`: actual static worker observation.
- `promoted-composition-profile-tests`: 12 pure legacy/promoted profile groups;
  does not call the worker.

Each observation keeps the original 110-second limit, bounded inputs, at most
16 functions / 128 blocks / 1,024 instructions, a 1-MiB artifact and a 2-MiB
report. Process exit zero means observation completed; callers must also require
`authored_interval_observation_complete`. Unsupported native effects remain
incomplete. A retained output file alone is not acceptance.

See [the dated source/native qualification](../../../../docs/ordered-composition-promoted-qualification-20260924.md)
for six actual promoted cases, compatibility with all fourteen original cases,
mutation coverage, retained evidence and remaining limitations.

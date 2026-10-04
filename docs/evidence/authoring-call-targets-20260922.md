# Exact static helper calls: qualification on 2026-09-22

This additive diagnostic increment advances #280 helper/call-site inspection and
#282 source readmission correspondence. M1/V1/U1 remain accepted (3/18); M2 and
U2 remain open. No existing snapshot, operation, Bundle V6 or canonical V11
schema is changed.

## Implemented query

[inspect_call_target_v1](../../crates/fe2o3-source-isa-observation/src/multilevel_authoring_call_target_v1.rs)
and the normal `fe2o3-author call-target --selector JSON` command consume the
same exact verified bundle and one selected direct Call. The immutable owner
supplies the actual caller role, complete bounded matching Kernel.entry
registrations, exact internal-helper FunctionId/ordinal, positional typed
operand/formal bindings and caller result/signature slots.

The caller is a registered kernel entry or a defined internal helper. Device-FFI
export roles are not representable in the current V11 payload; those and external
callers are refused. The callee must be a defined internal helper. The report
does not infer a callee from display names, specialization hashes or constant bits.
SSA IDs remain function-local. A result-signature slot is not a callee Return SSA
mapping. Static call sites are not dynamic invocations or physical helper ABI.

The query bounds values and registrations at 64, selected identities at 4096 bytes,
combined comparison work at 1048576 and serialized output at 256 KiB.
No body clone, executable rewrite, transitive helper traversal, proof admission,
compiler resume or launch capability is introduced. Existing operations JSON is
unchanged; the new correspondence has its own schema.

## Fresh source and actual normal queries

Each fork ran the unchanged [const-u32 source ladder](../../scripts/const-u32-helper-source-smoke.mjs)
with its separately built current tools: baseline, default256, edited512, exact
repeat, and two specializations. Each passed five actual exports, 150 fresh CPU simulations, 169 command stages,
three exact CLI refusals and 455 retained pins..

The new [call-target acceptance](../../scripts/call-target-source-acceptance.mjs)
then re-observed those exact bundles through normal CLI inspect, complete operation
paging and five Call queries. It checked actual selected simulator KernelId against
the retained caller registration, typed argument/formal/result slots, direct
256/512 helper constants, complete finite helper bodies, exact source call spans
and the observed caller input/result flow. No ordinal or name was guessed.

Canonical source receipt: 417968 bytes,
`bd58e82fcca08d35c8239611b85af8f46ce8090b6ac97ce0f017bc098b66ec4b`.
Canonical call receipt: 273153 bytes,
`0110e5c6d0e60bb557fd7ae29703e50ac8d7a110df71b51a852cc7578844f905`.
Mirror source receipt: 424539 bytes,
`9e99bc8f6b6133a674a5ccb1f28df10f3cf1d3877c2ddec06c1b9bd374e51840`.
Mirror call receipt: 277010 bytes,
`8c7394fd9dbdf6afde92cb6abef5f437a43fe44ef3e180fdfdfaf08b352ab700`..

The canonical query run passed 23 CLI stages, five call queries and seven exact
refusals. It retained 458 selected pins / 134950780 bytes, with 439 capture-owned
pins / 747856 bytes. The mirror passed the same 23/5/7 query/refusal counts with 458 selected pins /
134957906 bytes and 439 capture-owned pins / 748411 bytes..
Each query run revalidated 75 retained request files and 150 retained raw result
streams against the unchanged complete-buffer/initializedness/canary oracle.
It executed **zero new source exports or simulations**. Those executions belong
to the separately retained source-ladder receipt, not to a query-only receipt.

The original Phase20 receipts still correctly report that their public operation
projection lacked call targets. They were not rewritten. This new report supplies
a separately observed exact static relationship. Baseline has no Call; its kernel
registration is not inferred from the absence of a query.

For the canonical two-specialization capture only:

| Actual Call coordinate | Exact helper ordinal / typed constant | Caller operand to helper formal | Caller result slot |
| --- | --- | --- | --- |
| 0:0:3 | 1 / 256 | v18 to v1 | v19 / u32 |
| 0:1:0 | 2 / 512 | v3 to v2 | v20 / u32 |

These are observed logical IDs, not fixed API coordinates or physical VGPRs.
Rediscover them after every compilation. Both Calls belong to the actually
registered bitwise_chain kernel entry. The helper result-signature slot does
not identify a callee Return SSA value.

## Regression gates and exact boundaries

Both candidate bases are the Phase20 published mains:
canonical `6c5d00508c2c743d0984cd84f7a46b1fc2bebb60`,
mirror `c7756e1456a1df014a19ef48bf098df085feed5b`.
Working source censuses, not those base commits alone, contain this increment:

- Canonical: 6567 files / 99924840 bytes,
  `eceb06db774f29636a6bfb1fe1d9bb1d6a3713eec40448ac382fce9f24157999`.
- Mirror: 6560 files / 99860441 bytes,
  `2541cd0b661ee2fb26c3ad9bc7d184932291cfa91b63c09008b54573b99a6787`.

Both forks passed **164 authoring tests** (134 library, 6 author CLI, and
16/1/7 existing CLI suites), with no failures or ignores; each passed 128 Node
controls (108 existing plus 20 new). Formatting passed. Strict Clippy for the
authoring library and normal author CLI passed with `-D warnings`.
The frozen canonical backend harness passed **1711 tests / 110 ignored**;
the independently built mirror harness passed **1710 / 110 ignored**.
Ignored tests are not counted as passes. This is not whole-workspace strict
Clippy qualification; pre-existing broader backend warnings remain.

Retained supervisor receipts:

- Canonical build R2: `1d0974d2b2cc6eade52d676f5f64aedc9921a743e5c96f64ff9299f32266c604`.
- Mirror build R2: `8cd8a8f0d50cae5bb9d156a484efd21accadcbb9c30cabbf0df2a034726a59a1`.
- Authoring lint/controls: `6f9632738fd876df404983df865bb7ab497c47484a3f222eee8a822ed2aa0631`.
- Canonical backend: `9dafa63dec6c07f8eb848ea14d16b73ffbd36f32acf30f31d80575fc06b3d4e3`.
- Mirror backend: `3a4b68e99b459a6d11c908d9ef9c2001618d2c385110fb06636ce8ccb3e7e148`.
Later qualification/tutorial edits have separately recorded publication censuses.
Frozen executables and retained evidence are not rebuilt or relabeled in place.

The first compiler build's test fixture tried to encode a device-FFI export role
in canonical V11 and failed before reaching the intended query. That failure is
retained (receipt `94060be043c758fb2c5dc7c5b12c379ba359c57e90c4e21614c0537dd05bcee4`).
The new query, pure validators and tutorial were narrowed to actual admitted
caller roles; private corruption tests explicitly check unsupported-role refusal.
No codec gate was weakened and no failed run became a pass.

The [tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/retained-call-targets-v1.md)
covers exact bundle stdin, discovery/paging, caller registrations, typed bindings
and stale identities. It is a focused Markdown lab, not a newly qualified shared
curriculum lesson or compiler-pin/maturity promotion.

This is CPU diagnostic evidence, not authenticated Rust-source registration,
applicable protected proof invalidation, transitive helper closure, physical
clobber/lifetime/ABI correctness, native correctness or hardware execution.
The unrelated bounded-repeat and ordinary-source debugger-fault drafts are not
part of this increment. Task-local supervision retains the same 112 GiB total
root limit, >=40 GiB disk free, >=64 GiB available RAM, serialized gates and
offline/locked two-job builds; sampled limits are not hard process RSS guarantees.

# Inactive gfx950 debug preparation

This engineering-only API prepares real VM/mapping resources for a later debug-runtime implementation. It does **not** install a trap, enable a debug runtime, publish a code-object list, create a queue, dispatch a kernel or observe registers. It is not a stopped-wave qualification or a protected/source capability.

Use only in a separately supervised, disposable Linux x86_64 little-endian process. Once VM acquisition may have begun, error, unwind and Drop retain resources until process exit and poison the process-global reservation. There is no retry, close, activation or native teardown acknowledgment API.

## Ownership boundary

`Gfx950DebugColdOwnerV1::prepare` consumes a genuine `CheckedGfx950XnackMinusDevice`, owned ELF bytes and an entry name. It reuses the ordinary gfx950/COV6 loader and allocation/mapping mechanisms, retains exactly one genuine Kernel, a fixed reviewed trap-text mapping and owned version-zero debugger metadata, and performs actual currentness fences. Caller-provided metadata, trap bytes, native pointers and load biases are not accepted.

The normal `Context::open` path is unchanged. Preparation marks the exclusive gate exposed **before** acquiring the VM; this means possible native effects, not successful trap/runtime publication. Full actual native resources, original ELF and metadata are retained together on uncertainty. No metadata ADD notification occurs.

The fixed trap text is CPU-read-only after copying, and readback includes the zero-filled page tail. This does not remove GPU write permission. The loader bounds ELF and materialized image span to 64 MiB each; the trap requires one page and metadata retains one extra bounded ELF copy. Counts are logical storage, not RSS or allocator capacity.

`owner.facts()` exposes only preparation-time digests and byte counts. It does not revalidate currentness or confer authority to use a native address.

## Explicit one-shot example

Build the example with the engineering feature:

```sh
cargo build -p fe2o3-kfd --features engineering-gfx950 \
  --example observe_gfx950_cold_debug_v1
```

Under an external supervisor, use the exact closed grammar:

```text
observe_gfx950_cold_debug_v1 --allow-vm-mapping --retain-until-process-exit \
  ABSOLUTE_CANONICAL_HSACO_PATH BYTES SHA256 KERNEL NODE UNIQUE_ID GPU_ID DEVICE_PROFILE_SHA256
```

Both flags are mandatory and acknowledge real VM/allocation/mapping effects and process-lifetime retention. Numeric identities are canonical decimal; hashes are exactly 64 lowercase hexadecimal characters. The example accepts one explicit device, never a default-first GPU. Obtain and independently select the device/profile and artifact pins; a supplied hash is an integrity selection, not producer authentication.

The example performs bounded no-follow file reads, checks exact byte count/digest, retains its FD and immutable owned snapshot, and rechecks path/FD identity, metadata and bytes. Normal loader selection precedes device opening. The actual device must match node, unique ID, GPU ID and observation-profile digest; actual currentness is checked again before ownership transfers. The owner closes its own preparation currentness fence. A final artifact recheck and exact returned artifact/trap identity join precede a flushed JSON record of at most 4096 bytes.

This is not atomic filesystem authentication. A refusal after entering cold preparation conservatively reports possible retained native effects; it never claims clean teardown. Successful output states `prepared_inactive`, metadata version zero and false activation/dispatch/cleanup fields. Those flags describe this preparation path, not a survey of unrelated system activity. Only the external supervisor can establish process termination/reaping.

## Trap provenance and redistribution

The retained assembly is AMD ROCr source at commit
`820d83572bd8a098ba8366b84943c760ecce8ca5`, built offline for compute942 with explicitly selected ROCm 7.2.1 LLVM tools and `--no-default-config`. Its upstream gfx950 selector relation is source evidence, not executed trap compatibility.

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| Original upstream trap assembly | 40247 | ef70938c12e3811ffb23017bee10970dae0f933d93d35f8f9a14f082a72f10cf |
| Repository copy (one trailing comment space removed) | 40246 | b1b3db313fa622044a5682fff02f0492d2d7fecaf1d0b5309edda08807b21e61 |
| Offline ELF | 5992 | 2e5ce27b7082ce05974e1fb4dbf357696fdb49d8bad04191087be8cd18ac08a5 |
| Extracted trap text | 1116 | 4ffea893ee53e018a629c19254855721882444517155251f1f45dd3519bc81fe |

The full original source, AMD 2014–2024 copyright and University of Illinois/NCSA license are retained in [runtime_debug_trap_source_v1.s](../crates/fe2o3-kfd/src/runtime_debug_trap_source_v1.s). Source redistribution must preserve those notices; binary redistribution must reproduce the same copyright, conditions and disclaimer in accompanying documentation or materials. Ship that full retained source/license with a binary that includes the trap array. No AMD endorsement is implied.

The offline structural validator rejects relocation/dependency/section-shape deviations and compares extracted bytes with the independently retained text. It does not decode every instruction or prove execution safety. Upstream PC-sampling paths use TMA; no TMA-zero safety, sampling exclusion or live handler compatibility is established by this preparation API.

Qualification of this owner/example is separate from the earlier [read-only device/artifact companion](evidence/gfx950-checked-artifact-20260923.md). The [dated inactive-preparation qualification](evidence/gfx950-cold-debug-preparation-20260923.md) records one actual supervised VM/mapping preparation and seven phase-exact early refusals, plus the separately scoped library/example/lint gates and retained failures. It does not qualify trap registration/execution, runtime or metadata publication, queue creation, dispatch, stopped-wave capture or explicit native cleanup acknowledgment. Preparation-time facts remain historical and inert.

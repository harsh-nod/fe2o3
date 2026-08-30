# Debugger and profiler task matrix V1

Status: pinned comparison record for GitHub issue #215.

This matrix compares user tasks and authenticated evidence, not feature names.
It separates the capability of a collection substrate from the capability
currently admitted by fe2o3. A tool being able to print a value does not make
that value joinable to an exact fe2o3 source, KIR, artifact, dispatch, or
property identity.

The local qualification pins remain in
[`debugger-profiler-architecture-v1.md`](debugger-profiler-architecture-v1.md).
The external rows below use the public documentation available on 2026-08-29:

- [ROCgdb overview](https://rocm.docs.amd.com/projects/ROCgdb/en/latest/)
  and [essential commands](https://rocm.docs.amd.com/projects/ROCgdb/en/latest/quick-reference/essential-commands.html);
- [ROCprofiler-SDK quick reference](https://rocm.docs.amd.com/projects/rocprofiler-sdk/en/latest/quick-reference/quick_guide.html)
  and [thread trace](https://rocm.docs.amd.com/projects/rocprofiler-sdk/en/latest/how-to/using-thread-trace.html);
- [ROCprof Compute Viewer](https://rocm.docs.amd.com/projects/rocprof-compute-viewer/en/amd-mainline/how-to/using_compute_viewer.html);
  and
- [Mojo GPU debugging](https://docs.modular.com/mojo/tools/gpu-debugging/).

## Task comparison

| Task | ROCgdb | rocprofv3 / Compute Viewer | Mojo GPU debugger | fe2o3 admitted status |
| --- | --- | --- | --- | --- |
| Stop a live AMD GPU wave and inspect PC, registers, lanes, and target memory | Native hardware debugger; documented | Not an interactive debugger | Unsupported on AMD; documented GPU path uses CUDA-GDB on NVIDIA | V4 strictly correlates cooperative KFD publication with the five structured ROCgdb hierarchy commands and redacted lane/workgroup coordinates; registers, source, memory, and broad live-kernel acceptance remain unavailable |
| Break and step source on live hardware | Source breakpoints and single-step are documented | Not an interactive debugger | Breakpoints through CUDA-GDB; GPU stepping is documented as unreliable | Simulator source stepping is admitted; live fe2o3 source stepping remains unavailable |
| Reproduce a kernel fault without a GPU | Not its role | Not its role | GPU debugging requires supported NVIDIA hardware | Deterministic canonical KIR/Bundle replay, schedule control, source queries, and structured diagnosis are admitted; GPU timing is not predicted |
| Inspect source, work item, wave, workgroup, KIR, schedule, LLVM, and ISA as one identity graph | Native source and machine state, without fe2o3 semantic identities | Hardware timelines and samples, without fe2o3 source/KIR identities | Source and CUDA-GDB focus, without fe2o3 IR identities | Versioned semantic graph exists; production source-to-KIR coverage is partial and general schedule/LLVM/ISA round trips remain open |
| Collect dispatch intervals, counters, PC samples, runtime/copy events, and ATT | Not its primary profiling role | Native collector; rocprofv3 documents each class and Compute Viewer visualizes decoded ATT | Not documented as one comparable profiler contract | Strict dispatch/counter/PC and ATT-reference admission exists; complete decoded ATT/runtime/copy attribution remains open |
| Compare two captures without confusing process-local IDs | Manual debugger analysis | Collector output contains capture-local records; higher-level comparison is external | No documented cross-capture evidence contract | Bundle V4 retains exact same-KIR/artifact joins; isolated Variant V1 content-binds each treatment to exact supplied rocprofv3 bytes and maps Counter/PC evidence by process-local raw dispatch ID before fixing semantic workload, environment, tool/configuration, stable per-dispatch device, workload, and launch; compiled evidence may differ, while semantic/ISA localization and causal attribution remain unavailable |
| Cite every answer to immutable evidence and disclose loss, sampling, truncation, and unavailable state | Interactive debugger state, not a fe2o3 evidence protocol | Structured output exposes collector data and loss; it is not a fe2o3 semantic join | Interactive debugger workflow | This is the primary fe2o3 differentiator; all closure fixtures must retain stable evidence IDs for every material claim |
| Let a non-privileged agent query evidence without gaining launch, attach, pause, or recapture authority | MI can be automated but debugger control carries process authority | CLI/output can be automated; collection is a separate privileged action | CLI or VS Code workflow | Bounded read-only JSONL service exists; fresh-client diagnosis, citation, and minimum-capture planning acceptance remains under qualification |
| Localize an eight-GPU communication/computation overlap regression | Possible low-level manual debugging, not a distributed semantic graph | Can collect component traces subject to clock and loss limits | Not an AMD multi-GPU path | Typed unavailable until #182 supplies admitted operation, transfer, collective, and clock-correlation identities |

## Differentiator boundary

fe2o3 can be better at explaining *which admitted semantic claim a piece of
evidence supports*. It cannot claim deeper native machine visibility than
ROCgdb or richer decoded thread-trace analysis than rocprofv3 and Compute
Viewer until the corresponding producer, admission, query, hostile, and
hardware acceptance records pass.

The intended composed workflow is:

```text
compiler semantic map + artifact identity
  + deterministic simulator evidence
  + direct-KFD runtime identity
  + ROCgdb native observations when authenticated
  + rocprofv3 / decoded ATT observations when admitted
  -> bounded evidence-linked diagnosis and minimum next-capture plan
```

Collector output remains observation. It grants no compiler, artifact, proof,
load, launch, attach, pause, or dispatch authority. CPU performance prediction
is outside this issue.

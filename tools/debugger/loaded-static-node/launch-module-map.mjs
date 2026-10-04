// Fixed logical source/import roster; it is not a loader-IO observation or self-pin.
import {fileURLToPath} from 'node:url';
const MODULES=[
  "loaded-static-node/launch-main.mjs",
  "loaded-static-node/launch-fs.mjs",
  "loaded-static-node/launch-model.mjs",
  "loaded-operational-graph/loaded-operational-graph.mjs",
  "loaded-operational-graph/loaded-operational-policy.mjs",
  "loaded-cpu-adapter/adapter-guard.mjs",
  "loaded-cpu-adapter/adapter-protocol.mjs",
  "loaded-input-reader/reader-protocol.mjs",
  "loaded-cpu-adapter/adapter-writer.mjs",
  "loaded-static-node/launch-run.mjs",
  "loaded-static-node/launch-reader.mjs",
  "loaded-static-node/launch-module-map.mjs",
  "loaded-input-reader/reader-fs.mjs",
  "loaded-input-reader/reader-plan.mjs",
  "loaded-profile/loaded-selection.mjs",
  "loaded-profile/loaded-profile.mjs",
  "loaded-profile/loaded-profile-binding.mjs",
  "loaded-profile/loaded-observation-values.mjs",
  "loaded-profile/loaded-mi2-values.mjs",
  "loaded-profile/loaded-selection-binding.mjs",
  "loaded-input-reader/reader-aliases.mjs",
  "loaded-cpu-adapter/adapter-fs.mjs"
];
const EDGES=[
  {
    "from": "loaded-profile/loaded-profile.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-profile/loaded-profile.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-profile/loaded-profile.mjs",
    "specifier": "./loaded-profile-binding.mjs",
    "to": "loaded-profile/loaded-profile-binding.mjs"
  },
  {
    "from": "loaded-profile/loaded-profile.mjs",
    "specifier": "./loaded-observation-values.mjs",
    "to": "loaded-profile/loaded-observation-values.mjs"
  },
  {
    "from": "loaded-profile/loaded-profile.mjs",
    "specifier": "./loaded-mi2-values.mjs",
    "to": "loaded-profile/loaded-mi2-values.mjs"
  },
  {
    "from": "loaded-profile/loaded-observation-values.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-profile/loaded-observation-values.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-profile/loaded-observation-values.mjs",
    "specifier": "./loaded-profile-binding.mjs",
    "to": "loaded-profile/loaded-profile-binding.mjs"
  },
  {
    "from": "loaded-profile/loaded-mi2-values.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-profile/loaded-mi2-values.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-profile/loaded-selection.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-profile/loaded-selection.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-profile/loaded-selection.mjs",
    "specifier": "./loaded-profile.mjs",
    "to": "loaded-profile/loaded-profile.mjs"
  },
  {
    "from": "loaded-profile/loaded-selection.mjs",
    "specifier": "./loaded-profile-binding.mjs",
    "to": "loaded-profile/loaded-profile-binding.mjs"
  },
  {
    "from": "loaded-profile/loaded-selection.mjs",
    "specifier": "./loaded-selection-binding.mjs",
    "to": "loaded-profile/loaded-selection-binding.mjs"
  },
  {
    "from": "loaded-input-reader/reader-protocol.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-input-reader/reader-protocol.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-input-reader/reader-fs.mjs",
    "specifier": "node:fs",
    "to": null
  },
  {
    "from": "loaded-input-reader/reader-fs.mjs",
    "specifier": "./reader-plan.mjs",
    "to": "loaded-input-reader/reader-plan.mjs"
  },
  {
    "from": "loaded-input-reader/reader-fs.mjs",
    "specifier": "./reader-protocol.mjs",
    "to": "loaded-input-reader/reader-protocol.mjs"
  },
  {
    "from": "loaded-input-reader/reader-plan.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-input-reader/reader-plan.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-input-reader/reader-plan.mjs",
    "specifier": "../loaded-profile/loaded-selection.mjs",
    "to": "loaded-profile/loaded-selection.mjs"
  },
  {
    "from": "loaded-input-reader/reader-plan.mjs",
    "specifier": "./reader-aliases.mjs",
    "to": "loaded-input-reader/reader-aliases.mjs"
  },
  {
    "from": "loaded-input-reader/reader-plan.mjs",
    "specifier": "./reader-protocol.mjs",
    "to": "loaded-input-reader/reader-protocol.mjs"
  },
  {
    "from": "loaded-cpu-adapter/adapter-guard.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-cpu-adapter/adapter-writer.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-cpu-adapter/adapter-writer.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-cpu-adapter/adapter-writer.mjs",
    "specifier": "./adapter-guard.mjs",
    "to": "loaded-cpu-adapter/adapter-guard.mjs"
  },
  {
    "from": "loaded-cpu-adapter/adapter-protocol.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-cpu-adapter/adapter-protocol.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-cpu-adapter/adapter-protocol.mjs",
    "specifier": "../loaded-input-reader/reader-protocol.mjs",
    "to": "loaded-input-reader/reader-protocol.mjs"
  },
  {
    "from": "loaded-cpu-adapter/adapter-protocol.mjs",
    "specifier": "./adapter-guard.mjs",
    "to": "loaded-cpu-adapter/adapter-guard.mjs"
  },
  {
    "from": "loaded-cpu-adapter/adapter-fs.mjs",
    "specifier": "node:fs",
    "to": null
  },
  {
    "from": "loaded-cpu-adapter/adapter-fs.mjs",
    "specifier": "node:perf_hooks",
    "to": null
  },
  {
    "from": "loaded-cpu-adapter/adapter-fs.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-cpu-adapter/adapter-fs.mjs",
    "specifier": "../loaded-input-reader/reader-fs.mjs",
    "to": "loaded-input-reader/reader-fs.mjs"
  },
  {
    "from": "loaded-cpu-adapter/adapter-fs.mjs",
    "specifier": "./adapter-protocol.mjs",
    "to": "loaded-cpu-adapter/adapter-protocol.mjs"
  },
  {
    "from": "loaded-cpu-adapter/adapter-fs.mjs",
    "specifier": "./adapter-guard.mjs",
    "to": "loaded-cpu-adapter/adapter-guard.mjs"
  },
  {
    "from": "loaded-cpu-adapter/adapter-fs.mjs",
    "specifier": "./adapter-writer.mjs",
    "to": "loaded-cpu-adapter/adapter-writer.mjs"
  },
  {
    "from": "loaded-operational-graph/loaded-operational-graph.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-operational-graph/loaded-operational-graph.mjs",
    "specifier": "node:path",
    "to": null
  },
  {
    "from": "loaded-operational-graph/loaded-operational-graph.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-operational-graph/loaded-operational-policy.mjs",
    "specifier": "./loaded-operational-graph.mjs",
    "to": "loaded-operational-graph/loaded-operational-graph.mjs"
  },
  {
    "from": "loaded-static-node/launch-model.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-static-node/launch-model.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-static-node/launch-model.mjs",
    "specifier": "../loaded-operational-graph/loaded-operational-graph.mjs",
    "to": "loaded-operational-graph/loaded-operational-graph.mjs"
  },
  {
    "from": "loaded-static-node/launch-model.mjs",
    "specifier": "../loaded-operational-graph/loaded-operational-policy.mjs",
    "to": "loaded-operational-graph/loaded-operational-policy.mjs"
  },
  {
    "from": "loaded-static-node/launch-model.mjs",
    "specifier": "../loaded-cpu-adapter/adapter-guard.mjs",
    "to": "loaded-cpu-adapter/adapter-guard.mjs"
  },
  {
    "from": "loaded-static-node/launch-model.mjs",
    "specifier": "../loaded-cpu-adapter/adapter-protocol.mjs",
    "to": "loaded-cpu-adapter/adapter-protocol.mjs"
  },
  {
    "from": "loaded-static-node/launch-model.mjs",
    "specifier": "../loaded-cpu-adapter/adapter-writer.mjs",
    "to": "loaded-cpu-adapter/adapter-writer.mjs"
  },
  {
    "from": "loaded-static-node/launch-reader.mjs",
    "specifier": "node:crypto",
    "to": null
  },
  {
    "from": "loaded-static-node/launch-reader.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-static-node/launch-reader.mjs",
    "specifier": "../loaded-cpu-adapter/adapter-guard.mjs",
    "to": "loaded-cpu-adapter/adapter-guard.mjs"
  },
  {
    "from": "loaded-static-node/launch-reader.mjs",
    "specifier": "./launch-model.mjs",
    "to": "loaded-static-node/launch-model.mjs"
  },
  {
    "from": "loaded-static-node/launch-run.mjs",
    "specifier": "../loaded-cpu-adapter/adapter-guard.mjs",
    "to": "loaded-cpu-adapter/adapter-guard.mjs"
  },
  {
    "from": "loaded-static-node/launch-run.mjs",
    "specifier": "../loaded-cpu-adapter/adapter-writer.mjs",
    "to": "loaded-cpu-adapter/adapter-writer.mjs"
  },
  {
    "from": "loaded-static-node/launch-run.mjs",
    "specifier": "./launch-model.mjs",
    "to": "loaded-static-node/launch-model.mjs"
  },
  {
    "from": "loaded-static-node/launch-run.mjs",
    "specifier": "./launch-reader.mjs",
    "to": "loaded-static-node/launch-reader.mjs"
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "node:fs",
    "to": null
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "node:perf_hooks",
    "to": null
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "node:util",
    "to": null
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "node:url",
    "to": null
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "./launch-model.mjs",
    "to": "loaded-static-node/launch-model.mjs"
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "./launch-run.mjs",
    "to": "loaded-static-node/launch-run.mjs"
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "./launch-module-map.mjs",
    "to": "loaded-static-node/launch-module-map.mjs"
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "../loaded-input-reader/reader-fs.mjs",
    "to": "loaded-input-reader/reader-fs.mjs"
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "../loaded-cpu-adapter/adapter-fs.mjs",
    "to": "loaded-cpu-adapter/adapter-fs.mjs"
  },
  {
    "from": "loaded-static-node/launch-fs.mjs",
    "specifier": "../loaded-cpu-adapter/adapter-guard.mjs",
    "to": "loaded-cpu-adapter/adapter-guard.mjs"
  },
  {
    "from": "loaded-static-node/launch-main.mjs",
    "specifier": "./launch-fs.mjs",
    "to": "loaded-static-node/launch-fs.mjs"
  },
  {
    "from": "loaded-static-node/launch-module-map.mjs",
    "specifier": "node:url",
    "to": null
  }
];
const absolute=p=>fileURLToPath(new URL('../'+p,import.meta.url));
export function staticModuleContext(runtime_path){return {runtime_path,entry_path:absolute('loaded-static-node/launch-main.mjs'),module_paths:MODULES.map(absolute),import_edges:EDGES.map(e=>({from:absolute(e.from),specifier:e.specifier,to:e.to===null?runtime_path:absolute(e.to)}))};}

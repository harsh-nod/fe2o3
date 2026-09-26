import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compilerBoundarySuite, lifetimeDiagnostic } from './compiler-boundary-ui.mjs';
export { bounds } from './compiler-boundary-ui.mjs';

export const leaf = 'production_source_storage_root_custody_v29_ui.rs';
export const { cases, assess, cargoArguments, runSuite } = compilerBoundarySuite({
  leaf,
  selectors: Object.fromEntries(['positive', 'state', 'path', 'origins', 'view', 'owned', 'capture', 'install', 'plan', 'custody', 'recorded'].map(name => [name,
    ['fe2o3_source_storage_root_custody_ui', `fe2o3_source_storage_root_custody_ui_case="${name}"`],
  ])),
  expectedDiagnostic: (name, message) => name === 'owned'
    ? message.code?.code === 'E0308'
    : name === 'install'
      ? ['E0505', 'E0500', 'E0507'].includes(message.code?.code)
      : lifetimeDiagnostic(message),
});

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 3) throw new Error('usage: node scripts/source-storage-root-custody-ui.mjs ABSOLUTE_CHECKOUT');
  runSuite(resolve(process.argv[2]), process.env, { report: result => console.log(JSON.stringify(result)) });
}

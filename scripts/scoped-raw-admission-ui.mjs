import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compilerBoundarySuite, lifetimeDiagnostic } from './compiler-boundary-ui.mjs';
export { bounds } from './compiler-boundary-ui.mjs';

export const leaf = 'production_scoped_raw_admission_v29_ui.rs';
export const { cases, assess, cargoArguments, runSuite } = compilerBoundarySuite({
  leaf,
  selectors: Object.fromEntries(['positive', 'raw_graph', 'unfinished_graph', 'checked_path',
    'physical_escape', 'physical_capture', 'forged_completion'].map(name =>
    [name, name === 'positive' ? [] : [`fe2o3_raw_admission_ui_case="${name}"`]])),
  expectedDiagnostic: (name, message) => {
    if (name.startsWith('physical_')) return lifetimeDiagnostic(message);
    if (name === 'checked_path') return message.code?.code === 'E0308';
    if (name === 'forged_completion') return message.code?.code === 'E0451';
    return message.code?.code === 'E0616';
  },
});

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 3) throw new Error('usage: node scripts/scoped-raw-admission-ui.mjs ABSOLUTE_CHECKOUT');
  runSuite(resolve(process.argv[2]), process.env, { report: result => console.log(JSON.stringify(result)) });
}

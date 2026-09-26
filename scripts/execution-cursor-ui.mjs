import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compilerBoundarySuite, lifetimeDiagnostic } from './compiler-boundary-ui.mjs';
export { bounds } from './compiler-boundary-ui.mjs';

export const leaf = 'production_function_frame_scope_v1.rs';
export const { cases, assess, cargoArguments, runSuite } = compilerBoundarySuite({
  leaf,
  selectors: {
    positive: [],
    direct: ['fe2o3_cursor_escape_direct_v1'],
    nested: ['fe2o3_cursor_escape_nested_v1'],
    buffer_direct: ['fe2o3_cursor_buffer_escape_direct_v1'],
    buffer_nested: ['fe2o3_cursor_buffer_escape_nested_v1'],
  },
  expectedDiagnostic: (name, message) => name.startsWith('buffer_')
    ? message.code?.code === 'E0509'
    : lifetimeDiagnostic(message),
});

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length !== 3) throw new Error('usage: node scripts/execution-cursor-ui.mjs ABSOLUTE_CHECKOUT');
  runSuite(resolve(process.argv[2]), process.env, { report: result => console.log(JSON.stringify(result)) });
}

import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

export function bounds(source, name) {
  const lines = source.split('\n');
  const starts = lines.flatMap((line, index) => line === `// UI-BEGIN ${name}` ? [index + 1] : []);
  const ends = lines.flatMap((line, index) => line === `// UI-END ${name}` ? [index + 1] : []);
  if (starts.length !== 1 || ends.length !== 1 || ends[0] <= starts[0]) {
    throw new Error(`invalid UI marker pair: ${name}`);
  }
  return [starts[0], ends[0]];
}

export function lifetimeDiagnostic(message) {
  return ['E0521', 'E0515'].includes(message.code?.code)
    || (message.code === null && message.message.includes('lifetime may not live long enough'));
}

export function compilerBoundarySuite({ leaf, selectors, expectedDiagnostic }) {
  const cases = Object.keys(selectors);
  if (cases[0] !== 'positive') throw new Error('a production positive control must run first');
  const validate = name => {
    if (!Object.hasOwn(selectors, name)) throw new Error(`unknown UI case: ${name}`);
  };
  const library = row => row.target?.name === 'fe2o3_lower_mir_kernel' && row.target?.kind?.includes('lib');

  function assess(name, source, run) {
    validate(name);
    if (run.error || run.signal || !Number.isInteger(run.status)) throw new Error('compiler did not terminate normally');
    if (/internal compiler error|the compiler unexpectedly panicked|thread ['"].*panicked|signal: \d+|SIG(?:KILL|SEGV|ABRT)/i.test(run.stderr || '')) {
      throw new Error('compiler subprocess crashed');
    }
    const records = run.stdout.split('\n').filter(line => line.startsWith('{')).map(line => JSON.parse(line));
    const errors = records.filter(row => row.reason === 'compiler-message' && row.message?.level === 'error');
    const artifacts = records.filter(row => row.reason === 'compiler-artifact' && library(row));
    const finished = records.filter(row => row.reason === 'build-finished');
    if (finished.length !== 1 || finished[0].success !== (run.status === 0)) throw new Error('missing or inconsistent cargo terminal result');
    if (name === 'positive') {
      if (run.status !== 0 || errors.length !== 0 || artifacts.length === 0) throw new Error('positive production-signature control did not compile');
      return;
    }
    if (run.status !== 101 || errors.length === 0) throw new Error('negative production-signature control lacked a compiler refusal');
    const [start, end] = bounds(source, name);
    for (const row of errors) {
      const message = row.message;
      const targeted = library(row) && message.spans?.some(span => span.is_primary
        && span.file_name.replaceAll('\\', '/').endsWith(`/${leaf}`)
        && span.line_start > start && span.line_end < end && span.line_end >= span.line_start);
      if (!targeted || !expectedDiagnostic(name, message)) {
        throw new Error(`unrelated diagnostic cannot prove non-escape: ${message.message}`);
      }
    }
  }

  function cargoArguments(name) {
    validate(name);
    // These probes exercise type checking, not linking. Only the final package
    // receives the probe cfgs; dependencies retain their production build flags.
    return ['rustc', '--offline', '--locked', '--lib', '-p', 'fe2o3-lower-mir-kernel', '--message-format=json', '--', '--emit=metadata',
      ...selectors[name].flatMap(selector => ['--cfg', selector])];
  }

  function runSuite(repo, env = process.env, { runCompiler = spawnSync, report = () => {} } = {}) {
    const source = readFileSync(resolve(repo, 'crates/fe2o3-lower-mir-kernel/src', leaf), 'utf8');
    const reports = [];
    for (const name of cases) {
      const run = runCompiler(env.CARGO || 'cargo', cargoArguments(name), {
        cwd: repo, env: { ...env }, encoding: 'utf8', timeout: 900_000, maxBuffer: 64 * 1024 * 1024,
      });
      try {
        assess(name, source, run);
      } catch (error) {
        // Dependency artifacts can exhaust Node's uncaught-error display budget
        // before the actual diagnostic. Retain the bounded failure tail instead.
        throw new Error(`${name}: ${error.message} (status=${run.status}, signal=${run.signal}, error=${run.error?.message || 'none'})\n${(run.stdout || '').slice(-16_000)}\n${(run.stderr || '').slice(-16_000)}`);
      }
      const result = { case: name, status: name === 'positive' ? 'compiled' : 'rejected' };
      reports.push(result);
      report(result);
    }
    return reports;
  }

  return { cases, assess, cargoArguments, runSuite };
}

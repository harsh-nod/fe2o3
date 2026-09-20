// Private bounded stdin transport for the runtime-origin capture, not compiler authority.
import { spawn } from 'node:child_process';

export function runRuntimeOriginCommand({ executable, args, cwd, env, input = Buffer.alloc(0),
  timeoutMs = 30000, outputCap = 65536, guard = () => {} },
  { spawnImpl = spawn, killImpl = process.kill.bind(process) } = {}) {
  if (!Buffer.isBuffer(input) || input.length > 4 * 1024 * 1024
    || !Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 300000
    || !Number.isSafeInteger(outputCap) || outputCap < 1 || outputCap > 1024 * 1024) {
    throw new Error('runtime-origin process bounds');
  }
  return new Promise(resolve => {
    const start = performance.now(), retained = { stdout: [], stderr: [] };
    const sizes = { stdout: 0, stderr: 0 };
    let child, timer, monitor, reap, done = false, stopped = false, reason = null, offset = 0;
    const finish = (code, signal) => {
      if (done) return;
      done = true; stopped = true;
      clearTimeout(timer); clearInterval(monitor); clearTimeout(reap);
      child?.stdin?.removeListener('drain', feed);
      resolve({ code, signal, reason, elapsed_ms: Math.ceil(performance.now() - start),
        stdout: Buffer.concat(retained.stdout), stderr: Buffer.concat(retained.stderr) });
    };
    const stop = message => {
      reason ??= message;
      if (stopped) return;
      stopped = true;
      child?.stdin?.removeListener('drain', feed);
      child?.stdin?.destroy();
      if (child?.pid) {
        try { killImpl(-child.pid, 'SIGKILL'); }
        catch (error) { if (error.code !== 'ESRCH') reason += ';kill:' + (error.code ?? 'error'); }
      }
      // Failure to reap/drain can never produce a successful command record.
      reap = setTimeout(() => finish(null, null), 2000);
    };
    const feed = () => {
      if (stopped || done) return;
      try {
        while (offset < input.length) {
          const end = Math.min(offset + 16384, input.length), chunk = input.subarray(offset, end);
          offset = end;
          if (!child.stdin.write(chunk)) { child.stdin.once('drain', feed); return; }
        }
        child.stdin.end();
      } catch (error) { stop('stdin:' + (error.code ?? 'error')); }
    };
    try {
      guard();
      child = spawnImpl(executable, args, { cwd, env, detached: true, stdio: ['pipe', 'pipe', 'pipe'] });
      for (const stream of ['stdout', 'stderr']) {
        child[stream].on('data', bytes => {
          if (done) return;
          const remaining = outputCap - sizes[stream];
          if (remaining > 0) {
            const part = bytes.subarray(0, remaining);
            retained[stream].push(Buffer.from(part)); sizes[stream] += part.length;
          }
          if (bytes.length > remaining) stop(stream + '_cap');
        });
        child[stream].on('error', error => stop(stream + ':' + (error.code ?? 'error')));
      }
      child.stdin.on('error', error => stop('stdin:' + (error.code ?? 'error')));
      child.once('error', error => stop('spawn:' + (error.code ?? 'error')));
      child.once('close', finish);
      timer = setTimeout(() => stop('timeout'), timeoutMs);
      monitor = setInterval(() => { try { guard(); } catch { stop('resource_guard'); } }, 1000);
      feed();
    } catch (error) {
      reason = 'setup:' + (error.code ?? error.message);
      if (child) stop(reason); else finish(null, null);
    }
  });
}

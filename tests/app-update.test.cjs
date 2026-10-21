const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const ts = require('typescript');

// Exercise the real composable with IPC mocked; no WebView or installer is launched.
function controller(invoke) {
  const source = fs.readFileSync('src/composables/useAppUpdate.ts', 'utf8');
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } });
  const exports = {};
  vm.runInNewContext(outputText, { exports, Date, console, require: (name) => {
    if (name === '@tauri-apps/api/core') return { invoke, Channel: class {} };
    return require(name);
  } });
  return exports.useAppUpdate;
}

test('duplicate checks are ignored and state survives a new consumer', async () => {
  let complete;
  let calls = 0;
  const useUpdate = controller(() => { calls++; return new Promise(resolve => { complete = resolve; }); });
  const update = useUpdate();
  const pending = update.check();
  await update.check();
  assert.equal(calls, 1);
  assert.equal(useUpdate().state.phase, 'checking');
  complete({ version: '1.2.0', notes: '中文更新' });
  await pending;
  assert.equal(useUpdate().state.update.version, '1.2.0');
  assert.equal(update.state.phase, 'available');
});

test('download progress, retry reset and signature error remain visible', async () => {
  let channel;
  let rejectInstall;
  const update = controller((command, args) => command === 'app_update_check'
    ? Promise.resolve({ version: '1.2.0', notes: null })
    : new Promise((_, reject) => { channel = args.progress; rejectInstall = reject; }))();
  await update.check();
  const pending = update.install();
  channel.onmessage({ phase: 'downloading', downloaded: 50, total: 100, attempt: 1 });
  assert.equal(update.percent.value, 50);
  channel.onmessage({ phase: 'downloading', downloaded: 0, total: null, attempt: 2 });
  assert.equal(update.percent.value, null);
  assert.equal(update.state.attempt, 2);
  rejectInstall({ code: 'signature', detail: 'Invalid signature' });
  await pending;
  assert.equal(update.state.phase, 'error');
  assert.equal(update.state.errorCode, 'signature');
  assert.equal(update.busy.value, false);
  assert.equal(update.state.update.version, '1.2.0');
  channel.onmessage({ phase: 'downloading', downloaded: 99, total: 100, attempt: 2 });
  assert.equal(update.state.phase, 'error', 'late progress must not overwrite the terminal error');
});

test('failed recheck clears stale install metadata', async () => {
  let calls = 0;
  const update = controller(() => ++calls === 1
    ? Promise.resolve({ version: '1.2.0', notes: null })
    : Promise.reject({ code: 'timeout', detail: 'Timed out' }))();
  await update.check();
  await update.check();
  assert.equal(update.state.update, null);
  await update.install();
  assert.equal(calls, 2);
  assert.equal(update.state.errorCode, 'timeout');
});

test('no available release reports current without attempting installation', async () => {
  let calls = 0;
  const update = controller(() => { calls++; return Promise.resolve(null); })();
  await update.check();
  await update.install();
  assert.equal(update.state.phase, 'current');
  assert.equal(calls, 1);
});

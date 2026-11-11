// Unit-test worker lifecycle without running a browser or the interpreter.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const elements = new Map();
const element = id => {
  if (!elements.has(id)) elements.set(id, {
    value: '', textContent: '', hidden: false, disabled: false, handlers: {},
    addEventListener(name, callback) { this.handlers[name] = callback; },
    setAttribute() {}, removeAttribute() {}, focus() {},
  });
  return elements.get(id);
};
const workers = [];
class Worker {
  constructor(url, options) {
    assert.equal(url, './worker.js');
    assert.equal(options.type, 'module');
    workers.push(this);
  }
  postMessage(data) { this.data = data; }
  terminate() { this.terminated = true; }
}
let fetchFails = false;
let timeoutCallback;
const hello = 'genesis: praise 1 amen';
const context = {
  document: { getElementById: element }, Worker,
  localStorage: { getItem: () => null, setItem() {} },
  fetch: async url => {
    assert.match(url, /^\.\/pkg\/examples\/example\d+\.scripture$/);
    return { ok: !fetchFails, status: fetchFails ? 404 : 200, text: async () => hello };
  },
  setTimeout: callback => { timeoutCallback = callback; return 1; }, clearTimeout() {},
};
const initialized = vm.runInNewContext(readFileSync(new URL('../web/playground.js', import.meta.url), 'utf8'), context);
assert.equal(element('run').disabled, true);
await initialized;
assert.equal(element('source').value, hello);
assert.equal(element('run').disabled, false);
const click = id => element(id).handlers.click();
click('run');
assert.equal(element('run').disabled, true);
assert.equal(element('stop').disabled, false);
assert.equal(workers[0].data.source, hello);
assert.deepEqual(Object.keys(workers[0].data), ['source']);
click('stop');
assert.equal(workers[0].terminated, true);
assert.equal(element('status').textContent, 'Stopped');
workers[0].onmessage({ data: { output: ['stale'], error: null } });
assert.equal(element('status').textContent, 'Stopped');
assert.equal(element('output').textContent, '');
click('run');
workers[0].onerror({ preventDefault() {}, message: 'stale failure' });
assert.equal(element('status').textContent, 'Running');
workers[1].onmessage({ data: { output: ['before'], error: 'division by zero' } });
assert.equal(element('output').textContent, 'before');
assert.equal(element('error').textContent, 'division by zero');
assert.equal(element('run').disabled, false);
assert.equal(element('stop').disabled, true);
click('run');
workers[2].onmessage({ data: { output: [], error: null } });
assert.equal(element('placeholder').hidden, false);
assert.equal(element('status').textContent, 'Complete');
click('run');
timeoutCallback();
assert.equal(workers[3].terminated, true);
assert.match(element('error').textContent, /10 seconds/);
assert.equal(element('run').disabled, false);
element('example').value = 'functions';
fetchFails = true;
await click('load');
assert.match(element('error').textContent, /HTTP 404/);
assert.equal(element('source').value, hello);
assert.equal(element('source').disabled, false);
assert.equal(element('run').disabled, false);
fetchFails = false;
await click('load');
assert.equal(element('error').hidden, true);
assert.equal(element('status').textContent, 'Ready');
// The worker must await the async Wasm report before parsing or posting it.
const reports = [];
let resume;
const workerSelf = { postMessage: report => reports.push(report) };
const workerContext = {
  self: workerSelf,
  init: async () => {},
  run_script: () => new Promise(resolve => { resume = resolve; }),
};
const workerSource = readFileSync(new URL('../web/worker.js', import.meta.url), 'utf8')
  .replace(/^import .*;\n/, '');
vm.runInNewContext(workerSource, workerContext);
const pendingReport = workerSelf.onmessage({ data: { source: 'genesis: pray 0.01 amen' } });
await new Promise(setImmediate);
assert.equal(reports.length, 0);
resume(JSON.stringify({ output: ['after'], error: null }));
await pendingReport;
assert.equal(JSON.stringify(reports), JSON.stringify([{ output: ['after'], error: null }]));
console.log('Playground loading, cancellation, timeout, stale events, errors, empty output, and async worker reports passed.');

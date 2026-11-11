// Exercise the actual UI controller with a small DOM and Worker harness.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { lessons } from '../web/lessons.js';

const controller = readFileSync(new URL('../web/playground.js', import.meta.url), 'utf8').replace(/^import .*;\n/, '');
const storageKey = 'scripture-playground-v2';
function mount({ storage = new Map(), mobile = false, storageFails = false } = {}) {
  const elements = new Map();
  const element = id => {
    if (!elements.has(id)) elements.set(id, {
      value: '', textContent: '', hidden: false, disabled: false, open: false,
      scrollTop: 0, selectionStart: 0, selectionEnd: 0, handlers: {}, attributes: {}, dataset: {},
      addEventListener(name, callback) { this.handlers[name] = callback; },
      setAttribute(name, value) { this.attributes[name] = value; },
      removeAttribute(name) { delete this.attributes[name]; },
      focus() { this.focused = true; },
      scrollIntoView() { this.revealed = true; },
      setSelectionRange(start, end) { this.selectionStart = start; this.selectionEnd = end; },
      setRangeText(text, start, end) { this.value = this.value.slice(0, start) + text + this.value.slice(end); },
    });
    return elements.get(id);
  };
  const workers = [];
  let timeoutCallback;
  class Worker {
    constructor(url, options) {
      assert.equal(url, './worker.js');
      assert.equal(options.type, 'module');
      workers.push(this);
    }
    postMessage(data) { this.data = data; }
    terminate() { this.terminated = true; }
  }
  const context = {
    lessons, Worker,
    document: { getElementById: element, querySelector: element, body: { classList: { add() {} } } },
    localStorage: {
      getItem: key => { if (storageFails) throw Error('disabled'); return storage.get(key) ?? null; },
      setItem: (key, value) => { if (storageFails) throw Error('full'); storage.set(key, value); },
    },
    matchMedia: () => ({ matches: mobile }),
    setTimeout: callback => { timeoutCallback = callback; return 1; }, clearTimeout() {},
  };
  vm.runInNewContext(controller, context);
  const click = id => element(id).handlers.click();
  const edit = text => { element('source').value = text; element('source').handlers.input(); };
  const load = name => { element('example').value = name; click('load'); };
  return { element, workers, click, edit, load, storage, timeout: () => timeoutCallback() };
}

const ui = mount({ mobile: true });
const { element, workers, click } = ui;
assert.equal(element('source').value, lessons.hello.source);
assert.equal(element('lesson-title').textContent, lessons.hello.title);
click('run');
assert.equal(element('run').disabled, true);
assert.equal(element('stop').disabled, false);
assert.equal(element('workspace').dataset.view, 'output');
assert.equal(element('workspace').revealed, true);
assert.equal(workers[0].data.source, lessons.hello.source);
assert.deepEqual(Object.keys(workers[0].data), ['source']);
ui.load('values');
assert.equal(element('source').value, lessons.hello.source, 'cannot replace source during execution');
click('stop');
assert.equal(workers[0].terminated, true);
assert.equal(element('status').textContent, 'Stopped');
workers[0].onmessage({ data: { output: ['stale'], error: null } });
assert.equal(element('status').textContent, 'Stopped');
assert.equal(element('output').textContent, '');
click('run');
workers[0].onerror({ preventDefault() {}, message: 'stale failure' });
assert.equal(element('status').textContent, 'Running');
workers[1].onmessage({ data: { output: ['before'], error: 'line 2, column 5: division by zero' } });
assert.equal(element('output').textContent, 'before');
assert.match(element('error-summary').textContent, /cannot be divided by zero/);
assert.equal(element('error-details').open, false);
assert.equal(element('error-box').hidden, false);
assert.equal(element('run').disabled, false);
click('error-location');
assert.equal(element('workspace').dataset.view, 'source');
assert.equal(element('source').focused, true);
assert.equal(element('source').selectionStart, lessons.hello.source.indexOf('praise'));
ui.edit(lessons.hello.source + '\n');
assert.equal(element('error-location').hidden, true, 'old diagnostic must not jump into changed source');
assert.equal(element('source').attributes['aria-invalid'], undefined);
click('run');
workers[2].onmessage({ data: { output: [], error: null } });
assert.equal(element('placeholder').hidden, false);
assert.equal(element('status').textContent, 'Complete');
click('run');
ui.timeout();
assert.equal(workers[3].terminated, true);
assert.match(element('error').textContent, /10 seconds/);
assert.equal(element('run').disabled, false);

// Every example retains its own edits, including across reloads.
const ownCode = 'genesis:\n    praise "My experiment"\namen\n';
ui.edit(ownCode);
ui.load('values');
assert.equal(element('source').value, lessons.values.source);
ui.edit('let there be count = 9');
ui.load('functions');
ui.load('hello');
assert.equal(element('source').value, ownCode);
ui.load('values');
assert.equal(element('source').value, 'let there be count = 9');
const reopened = mount({ storage: ui.storage });
assert.equal(reopened.element('example').value, 'values');
assert.equal(reopened.element('source').value, 'let there be count = 9');
reopened.click('reset');
assert.equal(reopened.element('source').value, lessons.values.source);
const resetReload = mount({ storage: ui.storage });
resetReload.click('undo-load');
assert.equal(resetReload.element('source').value, 'let there be count = 9', 'undo survives a reload');
resetReload.load('functions');
resetReload.click('undo-load');
assert.equal(resetReload.element('example').value, 'values');
assert.equal(resetReload.element('source').value, 'let there be count = 9');
assert.equal(resetReload.element('draft-notice').hidden, true);

const migrated = mount({ storage: new Map([['scripture-playground-draft-v1', ownCode]]) });
assert.equal(migrated.element('example').value, 'custom');
assert.equal(migrated.element('source').value, ownCode);
assert.equal(migrated.element('expected-details').hidden, true);
assert.equal(mount({ storageFails: true }).element('draft-status').textContent, 'Not saved in this browser');
assert.equal(mount({ storage: new Map([[storageKey, '{broken']]) }).element('source').value, lessons.hello.source);
assert.equal(mount({ storage: new Map([[storageKey, JSON.stringify({ activeLesson: '__proto__', drafts: { hello: 123 } })]]) }).element('source').value, lessons.hello.source);

const next = mount();
next.click('run');
next.workers[0].onmessage({ data: { output: ['Hallelujah'], error: null } });
assert.equal(next.element('next-lesson').hidden, false);
next.edit('genesis: praise \"My greeting\" amen');
next.click('run');
next.workers.at(-1).onmessage({ data: { output: ['My greeting'], error: null } });
assert.equal(next.element('next-lesson').hidden, false, 'Successful experiments can continue to the next lesson');
next.click('next-lesson');
assert.equal(next.element('example').value, 'values');
assert.equal(next.element('workspace').dataset.view, 'source');
next.click('view-output');
assert.equal(next.element('view-output').attributes['aria-pressed'], 'true');
next.click('view-source');
assert.equal(next.element('view-output').attributes['aria-pressed'], 'false');
let prevented = false;
const key = (value, extra = {}) => next.element('source').handlers.keydown({ key: value, preventDefault() { prevented = true; }, ...extra });
key('Tab');
assert.equal(prevented, true);
key('Escape');
prevented = false;
key('Tab');
assert.equal(prevented, false, 'Escape then Tab must allow forward keyboard navigation');
key('Tab', { shiftKey: true });
assert.equal(prevented, false, 'Shift+Tab must retain native backwards navigation');

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
console.log('Guided lessons, draft recovery, mobile views, keyboard escape, diagnostics, worker lifecycle, and async reports passed.');

import { lessons } from './lessons.js';

const byId = id => document.getElementById(id);
const source = byId('source');
const output = byId('output');
const error = byId('error');
const placeholder = byId('placeholder');
const status = byId('status');
const runButton = byId('run');
const stopButton = byId('stop');
const loadButton = byId('load');
const selector = byId('example');
const storageKey = 'scripture-playground-v2';
const legacyKey = 'scripture-playground-draft-v1';
const drafts = Object.create(null);
let activeLesson = 'hello';
let previous = null;
let activeWorker = null;
let timeout = null;
let executedSource = null;
let errorPosition = null;
let escapeTab = false;
let storageAvailable = true;

function isLesson(name) { return Object.hasOwn(lessons, name); }
function validSource(value) { return typeof value === 'string' && value.length <= 131072; }
function updateLines() {
  byId('line-numbers').textContent = Array.from({ length: source.value.split('\n').length }, (_, i) => i + 1).join('\n');
  byId('line-numbers').scrollTop = source.scrollTop;
}
function saveDraft() {
  drafts[activeLesson] = source.value;
  try {
    localStorage.setItem(storageKey, JSON.stringify({ activeLesson, drafts, previous }));
    storageAvailable = true;
  } catch { storageAvailable = false; }
  const edited = source.value !== lessons[activeLesson].source;
  byId('draft-status').textContent = storageAvailable ? (edited ? 'Edited · saved locally' : 'Saved locally') : 'Not saved in this browser';
}
function setView(view, reveal = false) {
  byId('workspace').dataset.view = view;
  byId('view-source').setAttribute('aria-pressed', String(view === 'source'));
  byId('view-output').setAttribute('aria-pressed', String(view === 'output'));
  if (reveal && matchMedia('(max-width: 759px)').matches) {
    byId('workspace').scrollIntoView({ block: 'start', behavior: 'instant' });
  }
}
function renderLesson() {
  const lesson = lessons[activeLesson];
  selector.value = activeLesson;
  byId('lesson-title').textContent = lesson.title;
  byId('lesson-description').textContent = lesson.description;
  byId('lesson-challenge').textContent = lesson.challenge;
  lesson.concepts.forEach(([term, meaning], index) => {
    byId(`term-${index}`).textContent = term;
    byId(`meaning-${index}`).textContent = meaning;
  });
  byId('expected-details').hidden = lesson.output === null;
  byId('expected-details').open = false;
  byId('expected-output').textContent = lesson.output?.join('\n') || '';
  byId('reset').textContent = activeLesson === 'custom' ? 'Reset script' : 'Reset example';
  updateLines();
}
function clearResult() {
  output.textContent = '';
  error.textContent = '';
  error.hidden = true;
  byId('error-box').hidden = true;
  byId('error-details').open = false;
  byId('next-lesson').hidden = true;
  placeholder.hidden = false;
  placeholder.textContent = 'Run your code to see what it prints. Then try the experiment above.';
  status.textContent = 'Ready';
  source.removeAttribute('aria-invalid');
  executedSource = null;
  errorPosition = null;
}
function loadExample(name, reset = false) {
  if (activeWorker || !isLesson(name)) return;
  const nextSource = reset ? lessons[name].source : (drafts[name] ?? lessons[name].source);
  if (name === activeLesson && source.value === nextSource) {
    selector.value = activeLesson;
    return;
  }
  previous = { lesson: activeLesson, source: source.value };
  drafts[activeLesson] = source.value;
  activeLesson = name;
  source.value = nextSource;
  source.scrollTop = 0;
  renderLesson();
  clearResult();
  saveDraft();
  byId('draft-message').textContent = reset ? 'Original example restored. Your edit can still be recovered.' : 'Example opened. Your other edits are saved separately.';
  byId('draft-notice').hidden = false;
  setView('source');
}
function restorePrevious() {
  if (activeWorker || !previous) return;
  drafts[activeLesson] = source.value;
  activeLesson = previous.lesson;
  source.value = previous.source;
  previous = null;
  renderLesson();
  clearResult();
  saveDraft();
  byId('draft-notice').hidden = true;
  setView('source', true);
  source.focus();
}
function initialize() {
  try {
    const stored = JSON.parse(localStorage.getItem(storageKey) || 'null');
    if (stored && typeof stored === 'object' && stored.drafts && typeof stored.drafts === 'object') {
      for (const [name, value] of Object.entries(stored.drafts)) {
        if (isLesson(name) && validSource(value)) drafts[name] = value;
      }
      if (isLesson(stored.activeLesson)) activeLesson = stored.activeLesson;
      if (stored.previous && isLesson(stored.previous.lesson) && validSource(stored.previous.source)) previous = stored.previous;
    } else {
      const legacy = localStorage.getItem(legacyKey);
      if (validSource(legacy)) { activeLesson = 'custom'; drafts.custom = legacy; }
    }
  } catch { /* Keep the examples usable if storage is unavailable or damaged. */ }
  source.value = drafts[activeLesson] ?? lessons[activeLesson].source;
  renderLesson();
  saveDraft();
  if (previous) {
    byId('draft-message').textContent = 'You can still undo your last example change.';
    byId('draft-notice').hidden = false;
  }
  document.body.classList.add('enhanced');
}
function finish() {
  activeWorker?.terminate();
  activeWorker = null;
  clearTimeout(timeout);
  runButton.disabled = false;
  stopButton.disabled = true;
  loadButton.disabled = false;
  selector.disabled = false;
  byId('reset').disabled = false;
  byId('undo-load').disabled = false;
}
function showError(message) {
  error.textContent = message;
  error.hidden = false;
  byId('error-box').hidden = false;
  byId('error-details').open = false;
  placeholder.hidden = true;
  status.textContent = 'Needs attention';
  const location = message.match(/^line (\d+), column (\d+):/);
  errorPosition = location ? { line: Number(location[1]), column: Number(location[2]) } : null;
  const firstLine = message.split('\n')[0].replace(/^line \d+, column \d+:\s*/, '').replace(/; expected .*/, '');
  let summary = firstLine;
  if (message.includes('unexpected end of input')) summary = 'The script ended before it was complete. Check for a missing value or a closing amen.';
  else if (message.includes('unexpected ')) summary = 'There is something the interpreter cannot read here. Check the spelling, punctuation, and block endings.';
  else if (message.includes('division by zero')) summary = 'A number cannot be divided by zero. Change the divisor and try again.';
  else if (message.includes('undefined name')) summary = 'This name has not been declared. Check its spelling or introduce it with let there be.';
  byId('error-summary').textContent = (errorPosition ? `Line ${errorPosition.line}: ` : '') + summary;
  byId('error-location').hidden = !errorPosition || source.value !== executedSource;
  if (errorPosition) byId('error-location').textContent = `Go to line ${errorPosition.line}`;
}
function run() {
  if (activeWorker || runButton.disabled) return;
  clearResult();
  placeholder.textContent = 'Running your script…';
  status.textContent = 'Running';
  executedSource = source.value;
  runButton.disabled = true;
  stopButton.disabled = false;
  loadButton.disabled = true;
  selector.disabled = true;
  byId('reset').disabled = true;
  byId('undo-load').disabled = true;
  saveDraft();
  setView('output', true);
  try {
    const worker = new Worker('./worker.js', { type: 'module' });
    activeWorker = worker;
    worker.onmessage = ({ data }) => {
      if (activeWorker !== worker) return;
      output.textContent = data.output.join('\n');
      placeholder.hidden = data.output.length > 0 || Boolean(data.error);
      placeholder.textContent = 'The script finished without printing. Use praise to show a value.';
      if (data.error) {
        showError(data.error);
        if (source.value === executedSource) source.setAttribute('aria-invalid', 'true');
      } else {
        status.textContent = source.value === executedSource ? 'Complete' : 'Complete · earlier code';
        const next = lessons[activeLesson].next;
        if (next && source.value === executedSource) {
          byId('next-lesson').textContent = `Next: ${next === 'values' ? 'variables & numbers' : next === 'functions' ? 'functions' : 'a prophecy'}`;
          byId('next-lesson').hidden = false;
        }
      }
      finish();
    };
    worker.onerror = event => {
      event.preventDefault();
      if (activeWorker !== worker) return;
      showError(event.message || 'The interpreter could not start. Try a browser with WebAssembly and worker support.');
      finish();
    };
    activeWorker.postMessage({ source: executedSource });
    timeout = setTimeout(() => {
      finish();
      showError('Execution stopped after 10 seconds. Simplify the script and try again.');
    }, 10000);
  } catch (failure) {
    finish();
    showError(failure.message || String(failure));
  }
}
loadButton.addEventListener('click', () => loadExample(selector.value));
byId('reset').addEventListener('click', () => loadExample(activeLesson, true));
byId('undo-load').addEventListener('click', restorePrevious);
byId('next-lesson').addEventListener('click', () => {
  const next = lessons[activeLesson].next;
  if (next) { loadExample(next); byId('lesson-title').scrollIntoView({ block: 'start', behavior: 'instant' }); }
});
byId('view-source').addEventListener('click', () => setView('source', true));
byId('view-output').addEventListener('click', () => setView('output', true));
byId('error-location').addEventListener('click', () => {
  if (!errorPosition || source.value !== executedSource) return;
  const lines = source.value.split('\n');
  const before = lines.slice(0, errorPosition.line - 1).join('\n');
  const position = before.length + (errorPosition.line > 1 ? 1 : 0) + Array.from(lines[errorPosition.line - 1] || '').slice(0, errorPosition.column - 1).join('').length;
  setView('source', true);
  source.focus();
  source.setSelectionRange(position, position);
  source.scrollTop = Math.max(0, (errorPosition.line - 3) * 26);
  updateLines();
});
document.querySelector('.guide-link').addEventListener('click', () => { byId('syntax-guide').open = true; });
runButton.addEventListener('click', run);
stopButton.addEventListener('click', () => {
  finish();
  status.textContent = 'Stopped';
  placeholder.hidden = false;
  placeholder.textContent = 'Execution stopped. Your code is still here. Edit it and run again.';
});
function onEdit() {
  saveDraft();
  updateLines();
  source.removeAttribute('aria-invalid');
  byId('error-location').hidden = true;
  byId('next-lesson').hidden = true;
  if (executedSource !== null && !activeWorker) status.textContent = 'Edited since run';
}
source.addEventListener('input', onEdit);
source.addEventListener('scroll', () => { byId('line-numbers').scrollTop = source.scrollTop; });
source.addEventListener('keydown', event => {
  if (event.key === 'Escape') { escapeTab = true; return; }
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') { event.preventDefault(); run(); }
  if (event.key === 'Tab' && !event.shiftKey && !escapeTab) {
    event.preventDefault();
    source.setRangeText('    ', source.selectionStart, source.selectionEnd, 'end');
    onEdit();
  }
  escapeTab = false;
});
initialize();

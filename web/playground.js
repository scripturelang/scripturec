const source = document.getElementById('source');
const output = document.getElementById('output');
const error = document.getElementById('error');
const placeholder = document.getElementById('placeholder');
const status = document.getElementById('status');
const runButton = document.getElementById('run');
const stopButton = document.getElementById('stop');
const loadButton = document.getElementById('load');
const draftKey = 'scripture-playground-draft-v1';
let activeWorker = null;
let timeout = null;
const examples = { hello: 'example01.scripture', functions: 'example08.scripture', prophecy: 'example02.scripture' };
function saveDraft() { try { localStorage.setItem(draftKey, source.value); } catch { /* Keep editing without storage. */ } }
source.addEventListener('input', saveDraft);
loadButton.addEventListener('click', () => loadExample(document.getElementById('example').value));
async function loadExample(name) {
  source.disabled = true;
  runButton.disabled = true;
  loadButton.disabled = true;
  status.textContent = 'Loading example';
  try {
    const response = await fetch(`./pkg/examples/${examples[name]}`);
    if (!response.ok) throw new Error(`Could not load example (HTTP ${response.status}).`);
    source.value = await response.text();
    source.removeAttribute('aria-invalid');
    error.hidden = true;
    status.textContent = 'Ready';
    saveDraft();
  } catch (failure) {
    showError(failure.message || String(failure));
  } finally {
    source.disabled = false;
    runButton.disabled = false;
    loadButton.disabled = false;
  }
}
async function initialize() {
  try {
    const draft = localStorage.getItem(draftKey);
    if (draft !== null) { source.value = draft; return; }
  } catch { /* Storage is optional. */ }
  await loadExample('hello');
}
function finish() {
  activeWorker?.terminate();
  activeWorker = null;
  clearTimeout(timeout);
  runButton.disabled = false;
  stopButton.disabled = true;
  loadButton.disabled = false;
}
function showError(message) {
  error.textContent = message;
  error.hidden = false;
  placeholder.hidden = true;
  status.textContent = 'Error';
}
function run() {
  if (activeWorker || runButton.disabled) return;
  output.textContent = '';
  error.textContent = '';
  error.hidden = true;
  placeholder.hidden = false;
  placeholder.textContent = 'Running your script…';
  source.removeAttribute('aria-invalid');
  status.textContent = 'Running';
  runButton.disabled = true;
  stopButton.disabled = false;
  loadButton.disabled = true;
  saveDraft();
  try {
    const worker = new Worker('./worker.js', { type: 'module' });
    activeWorker = worker;
    worker.onmessage = ({ data }) => {
      if (activeWorker !== worker) return;
      output.textContent = data.output.join('\n');
      placeholder.hidden = data.output.length > 0 || Boolean(data.error);
      placeholder.textContent = 'Script completed without output.';
      if (data.error) {
        showError(data.error);
        source.setAttribute('aria-invalid', 'true');
      } else {
        status.textContent = 'Complete';
      }
      finish();
    };
    worker.onerror = event => {
      event.preventDefault();
      if (activeWorker !== worker) return;
      showError(event.message || 'The interpreter could not start. Try a browser with WebAssembly and worker support.');
      finish();
    };
    activeWorker.postMessage({ source: source.value });
    timeout = setTimeout(() => {
      finish();
      showError('Execution stopped after 10 seconds. Simplify the script and try again.');
    }, 10000);
  } catch (failure) {
    finish();
    showError(failure.message || String(failure));
  }
}
runButton.addEventListener('click', run);
stopButton.addEventListener('click', () => {
  finish();
  status.textContent = 'Stopped';
  placeholder.hidden = false;
  placeholder.textContent = 'Execution stopped. Edit your script and run it again.';
});
source.addEventListener('keydown', event => {
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') { event.preventDefault(); run(); }
  if (event.key === 'Tab' && !event.shiftKey) {
    event.preventDefault();
    source.setRangeText('    ', source.selectionStart, source.selectionEnd, 'end');
    saveDraft();
  }
});

initialize();

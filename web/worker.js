import init, { run_script } from './pkg/scripturec.js';

self.onmessage = async ({ data }) => {
  try {
    await init();
    self.postMessage(JSON.parse(await run_script(data.source)));
  } catch (error) {
    self.postMessage({ output: [], error: String(error?.message || error) });
  }
};

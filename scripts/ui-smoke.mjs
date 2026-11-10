// Real headless browser interactions against the mock UI, never the filesystem backend.
// Requires Node 22+ and Chrome. Run with Vite already serving localhost:1420.
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { join } from 'node:path';
import assert from 'node:assert/strict';

const scratch = process.env.JCODE_SCRATCH_DIR;
if (!scratch) throw new Error('Set JCODE_SCRATCH_DIR to a disposable scratch directory');
const profile = await mkdtemp(join(scratch, 'pelago-ui-test-'));
const chrome = spawn(process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', [
  '--headless=new', '--disable-gpu', '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank',
], { stdio: ['ignore', 'ignore', 'pipe'] });
let socket;
try {
  const endpoint = await new Promise((resolve, reject) => {
    let output = '';
    const timer = setTimeout(() => reject(new Error('Chrome startup timed out')), 15000);
    chrome.once('error', reject);
    chrome.stderr.on('data', chunk => {
      output += chunk;
      const match = output.match(/DevTools listening on (ws:\/\/[^\s]+)/);
      if (match) { clearTimeout(timer); resolve(match[1]); }
    });
  });
  socket = new WebSocket(endpoint);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let sequence = 0;
  const pending = new Map();
  socket.onmessage = ({ data }) => {
    const message = JSON.parse(data);
    const entry = pending.get(message.id);
    if (!entry) return;
    pending.delete(message.id);
    clearTimeout(entry.timer);
    if (message.error) entry.reject(new Error(message.error.message));
    else entry.resolve(message.result);
  };
  function call(method, params = {}, sessionId) {
    return new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timed out: ${method}`)); }, 15000);
      pending.set(id, { resolve, reject, timer });
      socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  }
  const { targetId } = await call('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await call('Target.attachToTarget', { targetId, flatten: true });
  const evaluate = async expression => {
    const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true }, sessionId);
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.text + ': ' + result.exceptionDetails.exception?.description);
    return result.result.value;
  };
  async function waitFor(expression) {
    for (let i = 0; i < 100; i++) {
      if (await evaluate(expression)) return;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw new Error(`UI condition not reached: ${expression}`);
  }
  async function scene(query) {
    await call('Page.navigate', { url: `http://localhost:1420/${query}` }, sessionId);
    await waitFor('document.querySelector(".modal h2") !== null');
    assert.equal(await evaluate('"__TAURI_INTERNALS__" in window'), false, 'Must only test mock backend');
  }
  async function type(selector, text) {
    await evaluate(`document.querySelector(${JSON.stringify(selector)}).focus(); document.querySelector(${JSON.stringify(selector)}).select()`);
    await call('Input.insertText', { text }, sessionId);
  }
  await scene('?open=park');
  await waitFor('document.body.innerText.includes("1,264")');
  assert.equal(await evaluate('document.querySelector(".modal-foot .btn:last-child").disabled'), true);
  await evaluate(`document.querySelectorAll('.parking-check input').forEach(input => input.click())`);
  assert.equal(await evaluate('document.querySelector(".modal-foot .btn:last-child").disabled'), true, 'Acknowledgments alone cannot authorize deletion');
  const passphrase = 'ocean island garden lantern compass sunrise';
  await type('#parking-passphrase', passphrase);
  await type('#parking-confirm', 'not the same passphrase');
  const projectName = await evaluate('document.querySelector(".modal h2").textContent.replace(/^Park /, "")');
  await type('#parking-delete', `DELETE ${projectName}`);
  assert.equal(await evaluate('document.querySelector(".modal-foot .btn:last-child").disabled'), true, 'Mismatched passphrases block removal');
  await type('#parking-confirm', passphrase);
  await waitFor('!document.querySelector(".modal-foot .btn:last-child").disabled');
  await evaluate('document.querySelector(".modal-foot .btn:last-child").click()');
  await waitFor('document.body.innerText.includes("Encrypting, uploading")');
  await waitFor('document.querySelector(".modal") === null');
  await scene('?open=restore');
  assert.equal(await evaluate('document.querySelector(".modal-foot .btn:last-child").disabled'), true, 'Restore requires a passphrase');
  await scene('?open=restore&fail=restore_project');
  await type('#parking-passphrase', passphrase);
  await evaluate('document.querySelector(".modal-foot .btn:last-child").click()');
  await waitFor('document.querySelector("[role=alert]") !== null');
  assert.equal(await evaluate('document.querySelector("#parking-passphrase").value'), '', 'Failed restores clear secrets');
  await scene('?open=park&vault=missing');
  await waitFor('document.body.innerText.includes("Configure a private vault")');
  assert.equal(await evaluate('document.querySelector(".modal-foot .btn:last-child").disabled'), true, 'Missing vault blocks parking');
  await scene('?open=park&fail=preview_park');
  await waitFor('document.querySelector("[role=alert]") !== null');
  assert.equal(await evaluate('document.querySelector(".modal-foot .btn:last-child").disabled'), true, 'Preview errors block parking');
  console.log('PASS: parking acknowledgments, passphrase gating, missing vault and preview failure in real headless Chrome');
} finally {
  socket?.close();
  chrome.kill('SIGTERM');
  await new Promise(resolve => chrome.exitCode !== null ? resolve() : chrome.once('exit', resolve));
  await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyAgentUiRequest } from '../../../frontend/src/components/agent-control/useCloudAgentBridge.ts';

function fixture() {
  const calls = [];
  const handlers = {
    busy: false, view: {},
    open: async id => calls.push(['open', id]),
    newMeeting: () => calls.push(['new']),
    prepare: draft => calls.push(['prepare', draft]),
    setFollow: value => calls.push(['follow', value]),
    setConnections: value => calls.push(['connections', value]),
    dismiss: value => calls.push(['dismiss', value]),
    copied: () => calls.push(['copied']),
  };
  return { calls, handlers };
}
const request = (action, args = {}) => ({ id: 'request', action, arguments: args });

test('agents cannot replace an active meeting view or alter its setup', async () => {
  const { calls, handlers } = fixture();
  for (const req of [request('new'), request('prepare', { microphone: 'different' }), request('open', { id: 'old' })]) {
    await assert.rejects(applyAgentUiRequest(req, handlers, async () => ({ id: 'live' })), /active meeting/);
  }
  assert.deepEqual(calls, []);
  await applyAgentUiRequest(request('open', { id: 'live' }), handlers, async () => ({ id: 'live' }));
  assert.deepEqual(calls, [['open', 'live']]);
});

test('acknowledgement waits until the existing open action completes', async () => {
  const { handlers } = fixture();
  let release;
  let done = false;
  handlers.open = () => new Promise(resolve => { release = resolve; });
  const result = applyAgentUiRequest(request('open', { id: 'saved' }), handlers, async () => null).then(() => { done = true; });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(done, false);
  release(); await result;
  assert.equal(done, true);
});

test('view actions use the same state callbacks as the human controls', async () => {
  const { calls, handlers } = fixture();
  for (const req of [request('prepare', { title: 'Plan', participants: ['Chad'], microphone: '' }),
    request('follow', { enabled: false }), request('connections', { open: true }),
    request('dismiss', { target: 'error' }), request('copied')]) {
    await applyAgentUiRequest(req, handlers, async () => null);
  }
  assert.deepEqual(calls, [
    ['prepare', { title: 'Plan', participants: ['Chad'], microphone: '' }],
    ['follow', false], ['connections', true], ['dismiss', 'error'], ['copied'],
  ]);
});

test('busy or unknown actions fail without acknowledging a change', async () => {
  const { calls, handlers } = fixture();
  handlers.busy = true;
  await assert.rejects(applyAgentUiRequest(request('new'), handlers, async () => null), /busy/);
  handlers.busy = false;
  await assert.rejects(applyAgentUiRequest(request('arbitrary_native_command'), handlers, async () => null), /Unsupported/);
  assert.deepEqual(calls, []);
});

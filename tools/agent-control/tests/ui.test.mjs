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
    playback: async args => calls.push(['playback', args]),
    closePlayback: () => calls.push(['playback_close']),
    workspace: args => calls.push(['workspace', args]),
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


test('playback uses the shared controller callback and is denied during capture', async () => {
  const {calls,handlers}=fixture();
  await assert.rejects(applyAgentUiRequest(request('playback',{id:'live',playing:true}),handlers,async()=>({id:'live'})),/active meeting/);
  assert.deepEqual(calls,[]);
  await applyAgentUiRequest(request('playback',{id:'saved',position:12,speed:2,playing:true}),handlers,async()=>null);
  await applyAgentUiRequest(request('playback_close'),handlers,async()=>null);
  await applyAgentUiRequest(request('workspace',{panel:'settings',trash:true}),handlers,async()=>null);
  assert.deepEqual(calls,[['playback',{id:'saved',position:12,speed:2,playing:true}],['playback_close'],['workspace',{panel:'settings',trash:true}]]);
});

test('agent outline callbacks return the shared result and await persistence; failures keep their cause', async()=>{
 const {handlers}=fixture();let finish;
 handlers.workspace=()=>new Promise(resolve=>finish=resolve);
 let settled=false;const task=applyAgentUiRequest(request('workspace',{action:'outline'}),handlers,async()=>null).then(value=>{settled=true;return value});
 await new Promise(resolve=>setImmediate(resolve));assert.equal(settled,false);
 finish({nodes:[{id:'section/plan/0',expanded:false}]});assert.deepEqual(await task,{nodes:[{id:'section/plan/0',expanded:false}]});
 handlers.workspace=async()=>{throw new Error('Outline section changed. Read the outline and retry.')};
 await assert.rejects(applyAgentUiRequest(request('workspace'),handlers,async()=>null),/section changed/);
});

import {test} from 'node:test';
import assert from 'node:assert/strict';
import {PlaybackController} from '../../../frontend/src/components/cloud/playback.ts';
class Audio extends EventTarget{
 src='';currentTime=0;playbackRate=1;paused=true;readyState=0;
 load(){queueMicrotask(()=>{this.readyState=1;this.dispatchEvent(new Event('loadedmetadata'));});}
 async play(){this.paused=false;}
 pause(){this.paused=true;}
}
function fixture(){const audio=new Audio();const states=[];const calls=[];const controller=new PlaybackController(audio,async(id,channel)=>{calls.push([id,channel]);return{meeting_id:id,path:'safe.wav',start:5,end:25,duration:20,channel,edit_version:1};},path=>'asset:'+path,state=>states.push(state));return{audio,states,calls,controller};}
test('playback uses original times over trimmed audio, changes speed, pauses, and ends',async()=>{const {audio,controller,states}=fixture();await controller.set({id:'one',position:8,speed:1.5,playing:true});assert.equal(audio.currentTime,3);assert.equal(audio.playbackRate,1.5);assert.equal(controller.state.status,'playing');assert.equal(states[0].status,'loading');await controller.set({id:'one',playing:false});assert.equal(audio.paused,true);audio.currentTime=10;audio.dispatchEvent(new Event('timeupdate'));assert.equal(controller.state.position,15);audio.dispatchEvent(new Event('ended'));assert.equal(controller.state.position,25);assert.equal(controller.state.status,'ended');controller.close();assert.equal(controller.state.id,null);});
test('switching channels keeps position and play state',async()=>{const {audio,controller,calls}=fixture();await controller.set({id:'one',position:12,playing:true});await controller.set({id:'one',channel:'me'});assert.equal(audio.currentTime,7);assert.equal(controller.state.status,'playing');assert.deepEqual(calls,[['one','both'],['one','me']]);});
test('invalid speeds, sources, and trimmed-out seek positions fail visibly',async()=>{const {controller}=fixture();await assert.rejects(controller.set({id:'one',speed:Infinity}));await assert.rejects(controller.set({id:'one',channel:'arbitrary'}));await controller.set({id:'one'});await assert.rejects(controller.set({id:'one',position:3}),/outside/);assert.equal(controller.state.status,'error');assert.match(controller.state.error,/outside/);await controller.set({id:'one',position:9,playing:true});assert.equal(controller.state.status,'playing');});
test('closing during an async load prevents stale playback',async()=>{const audio=new Audio();let finish;const controller=new PlaybackController(audio,()=>new Promise(r=>finish=r),x=>x,()=>{});const pending=controller.set({id:'one',playing:true});controller.close();finish({meeting_id:'one',path:'file',start:0,end:20,duration:20,channel:'both',edit_version:0});await pending;assert.equal(controller.state.id,null);assert.equal(audio.paused,true);});
test('loading failures are reported and retryable',async()=>{const audio=new Audio();let failed=true;const controller=new PlaybackController(audio,async()=>{if(failed)throw new Error('missing audio');return{meeting_id:'one',path:'file',start:0,end:20,duration:20,channel:'both',edit_version:0};},x=>x,()=>{});await assert.rejects(controller.set({id:'one',playing:true}),/missing audio/);assert.equal(controller.state.status,'error');failed=false;await controller.set({id:'one',playing:true});assert.equal(controller.state.status,'playing');});
test('opening a cited recording keeps its pending load; editing that recording invalidates audio',async()=>{
 const {controller,audio}=fixture();
 controller.observeRecording(null,0);
 const pending=controller.set({id:'one',position:8,playing:true});
 controller.observeRecording('one',12);
 await pending;assert.equal(controller.state.status,'playing');assert.equal(audio.currentTime,3);
 controller.observeRecording('one',13);assert.equal(controller.state.status,'idle');assert.equal(audio.paused,true);
 const another=controller.set({id:'two',position:10,playing:true});
 controller.observeRecording('two',30);
 await another;assert.equal(controller.state.id,'two');assert.equal(controller.state.status,'playing');
});

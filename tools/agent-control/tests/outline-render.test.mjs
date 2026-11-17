import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createRequire,registerHooks} from 'node:module';
import {existsSync,readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
const require=createRequire(new URL('../../../frontend/package.json',import.meta.url));
const ts=require('typescript');
registerHooks({
 resolve(specifier,context,next){
  if(specifier.startsWith('.') && !/\.(mjs|js|ts|tsx)$/.test(specifier)){
   for(const extension of ['.ts','.tsx']){const url=new URL(specifier+extension,context.parentURL);if(existsSync(url))return next(url.href,context);}
  }return next(specifier,context);
 },
 load(url,context,next){if(url.endsWith('.tsx'))return {format:'module',shortCircuit:true,source:ts.transpileModule(readFileSync(fileURLToPath(url),'utf8'),{compilerOptions:{jsx:ts.JsxEmit.ReactJSX,module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText};return next(url,context);}
});
const React=require('react');const {renderToStaticMarkup}=require('react-dom/server');
const {default:NotesOutline}=await import('../../../frontend/src/components/cloud/NotesOutline.tsx');
const {outlineDocument,emptyOutline}=await import('../../../frontend/src/components/cloud/outline.ts');
test('nested outline renders exactly the same identifiable controls the agent reads, without parent bullets',()=>{
 const doc=outlineDocument('## Plan\n- Launch\n  - Test\n  - Document\n- Share',emptyOutline);
 const html=renderToStaticMarkup(React.createElement(NotesOutline,{sections:doc.sections,state:emptyOutline,toggle:()=>{}}));
 assert.match(html,/class="cl-outline-parent"/);
 for(const node of doc.nodes)assert.ok(html.includes(`data-outline-id="${node.id}"`),node.id);
 assert.equal((html.match(/<details/g)||[]).length,doc.nodes.length);
 const state={default_collapsed:true,overrides:{}};
 const collapsed=renderToStaticMarkup(React.createElement(NotesOutline,{sections:doc.sections,state,toggle:()=>{}}));assert.doesNotMatch(collapsed,/open=""/);
});
const {default:NotesScroll}=await import('../../../frontend/src/components/cloud/NotesScroll.tsx');
test('notes refresh keeps the visible section in place when content above grows, with a scroll-position fallback',()=>{
 let anchorTop=100;let anchors=[];
 const anchor={dataset:{outlineId:'section/plan/0'},getBoundingClientRect:()=>({top:anchorTop,bottom:anchorTop+200})};
 anchors=[anchor];
 const viewport={scrollTop:400,getBoundingClientRect:()=>({top:80}),querySelectorAll:()=>anchors};
 const component=new NotesScroll({version:'after',children:null});component.viewport.current=viewport;
 const snapshot=component.getSnapshotBeforeUpdate({version:'before'});
 anchorTop=220;component.componentDidUpdate(null,null,snapshot);assert.equal(viewport.scrollTop,520);
 anchors=[];component.props={version:'next',children:null};
 const fallback=component.getSnapshotBeforeUpdate({version:'after'});viewport.scrollTop=0;
 component.componentDidUpdate(null,null,fallback);assert.equal(viewport.scrollTop,520);
 assert.equal(component.getSnapshotBeforeUpdate({version:'next'}),null);
});
const {default:TranscriptPane}=await import('../../../frontend/src/components/cloud/TranscriptPane.tsx');
const {transcriptGroups}=await import('../../../frontend/src/components/cloud/transcript.ts');
const {create,act}=require('react-test-renderer');
test('transcript sentences remain inline and keyboard playable; live capture disables playback',()=>{
 const rows=[{id:'a',turn_id:'a',speaker:'me',channel:'me',text:'A sentence continues',start:0,end:2,final:true},{id:'b',turn_id:'b',speaker:'me',channel:'me',text:'across a provider boundary.',start:2,end:4,final:true}];
 const calls=[];const props={meeting:{id:'fixture',names:{},duration:4},groups:transcriptGroups(rows),active:false,independent:true,follow:false,setFollow:()=>{},play:t=>calls.push(t),edit:()=>{},redact:()=>{}};
 let rendered;act(()=>{rendered=create(React.createElement(TranscriptPane,props));});
 let passages=rendered.root.findAllByProps({className:'cl-passage'});
 assert.equal(passages.length,2);assert.ok(passages.every(p=>p.type==='span'&&p.props.role==='button'&&p.props.tabIndex===0));
 passages[0].props.onClick();let prevented=false;passages[1].props.onKeyDown({key:'Enter',preventDefault:()=>{prevented=true;}});
 assert.deepEqual(calls,[0,2]);assert.ok(prevented);
 act(()=>rendered.update(React.createElement(TranscriptPane,{...props,active:true})));
 passages=rendered.root.findAllByProps({className:'cl-passage'});assert.ok(passages.every(p=>p.props['aria-disabled']&&p.props.tabIndex===-1));
 passages[0].props.onClick();assert.deepEqual(calls,[0,2]);act(()=>rendered.unmount());
});
const {default:QuickStart}=await import('../../../frontend/src/components/cloud/QuickStart.tsx');
test('quick start requires no meeting details and starts directly with the selected input',()=>{
 let starts=0;const microphones=[];
 const props={title:'',participants:[],microphone:'',devices:[{name:'MacBook Pro Microphone'}],busy:false,start:()=>starts++,selectMicrophone:value=>microphones.push(value),connections:()=>{}};
 let rendered;act(()=>{rendered=create(React.createElement(QuickStart,props));});
 assert.equal(rendered.root.findAllByType('input').length,0);assert.equal(rendered.root.findAllByType('textarea').length,0);
 const button=rendered.root.findAllByType('button').find(b=>b.props.className.includes('cl-quick-record'));button.props.onClick();assert.equal(starts,1);
 rendered.root.findByType('select').props.onChange({target:{value:'MacBook Pro Microphone'}});assert.deepEqual(microphones,['MacBook Pro Microphone']);
 act(()=>rendered.update(React.createElement(QuickStart,{...props,busy:true})));assert.equal(rendered.root.findAllByType('button').find(b=>b.props.className.includes('cl-quick-record')).props.disabled,true);
 act(()=>rendered.unmount());
});
const {default:CalendarPanel,CalendarPrompt,upcoming}=await import('../../../frontend/src/components/cloud/CalendarPanel.tsx');
test('calendar panel keeps account calendars distinct and call prompt never starts automatically',()=>{
 const account=id=>({id,email:`${id}@test`,selected:['primary'],calendars:[{id:`${id}@test`,name:'Work',primary:true}]});
 const state={configured:true,accounts:[account('one'),account('two')],events:[],account_errors:{two:'Reconnect this account.'},phase:'idle',error:null,last_sync:null,detection:{enabled:true,signals:[{id:'Chrome',app:'Chrome',evidence:'Chrome is using the microphone; this may be a call'}],error:null,last_scan:null}};
 const html=renderToStaticMarkup(React.createElement(CalendarPanel,{state,loadError:'',meeting:null,close:()=>{},changed:()=>{}}));assert.match(html,/one@test/);assert.match(html,/two@test/);assert.match(html,/Reconnect this account/);assert.equal((html.match(/>Work</g)||[]).length,2);
 let starts=0;let rendered;act(()=>{rendered=create(React.createElement(CalendarPrompt,{state,active:false,start:()=>starts++,open:()=>{},busy:false}));});assert.equal(starts,0);rendered.root.findAllByType('button').find(b=>b.props.children==='Record').props.onClick();assert.equal(starts,1);act(()=>rendered.update(React.createElement(CalendarPrompt,{state,active:true,start:()=>starts++,open:()=>{},busy:false})));assert.equal(rendered.toJSON(),null);act(()=>rendered.unmount());
 const now=Date.now();const event=(key,delta)=>({key,start:new Date(now+delta).toISOString(),end:new Date(now+delta+10000).toISOString()});assert.deepEqual(upcoming([event('past',-20000),event('later',5000),event('now',0),event('tomorrow',90000000)],now).map(e=>e.key),['now','later']);
});

const {ChatComposer,ChatCitations}=await import('../../../frontend/src/components/cloud/ChatControls.tsx');
test('chat Enter submits through the form; newline, composition, repeat, blank and busy are guarded',()=>{
 let sends=0;const props={question:'What changed?',setQuestion:()=>{},scope:'all',setScope:()=>{},currentAvailable:true,busy:false,error:'',submit:()=>sends++};
 let rendered;act(()=>{rendered=create(React.createElement(ChatComposer,props));});
 const press=(extra={})=>{let prevented=false;rendered.root.findByType('textarea').props.onKeyDown({key:'Enter',shiftKey:false,repeat:false,nativeEvent:{},preventDefault:()=>prevented=true,currentTarget:{form:{requestSubmit:()=>rendered.root.findByType('form').props.onSubmit({preventDefault:()=>{}})}},...extra});return prevented;};
 assert.equal(press(),true);assert.equal(sends,1);
 for(const extra of [{shiftKey:true},{nativeEvent:{isComposing:true}},{nativeEvent:{keyCode:229}},{key:'a'}])assert.equal(press(extra),false);
 assert.equal(sends,1);assert.equal(press({repeat:true}),true);assert.equal(sends,1);
 for(const extra of [{busy:true},{question:'   '}]){
  act(()=>rendered.update(React.createElement(ChatComposer,{...props,...extra})));
  assert.equal(rendered.root.findByType('button').props.disabled,true);press();rendered.root.findByType('form').props.onSubmit({preventDefault:()=>{}});assert.equal(sends,1);
 }
 act(()=>rendered.unmount());
});
test('chat shows only cited sources and opens their original audio positions',()=>{
 const sources=Array.from({length:80},(_,i)=>({meeting_id:`meeting-${i}`,title:`Call ${i+1}`,start:i*10,end:i*10+5,text:'Excerpt'}));
 const opened=[];let rendered;act(()=>{rendered=create(React.createElement(ChatCitations,{sources,indices:[57,79,58,100],open:source=>opened.push(source)}));});
 const buttons=rendered.root.findAllByType('button');assert.equal(buttons.length,3);
 assert.deepEqual(buttons.map(b=>b.props.children[1]),[57,79,58]);buttons[0].props.onClick();assert.deepEqual(opened,[sources[56]]);assert.equal(opened[0].start,560);
 act(()=>rendered.update(React.createElement(ChatCitations,{sources,indices:[],open:()=>{}})));assert.equal(rendered.root.findAllByType('button').length,0);act(()=>rendered.unmount());
});

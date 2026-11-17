import {test} from 'node:test';
import assert from 'node:assert/strict';
import {outline,resized,defaults,refreshRates} from '../../../frontend/src/components/cloud/workspace.ts';
import {transcriptRows,speakerName} from '../../../frontend/src/components/cloud/types.ts';
test('outline grows under stable topic IDs and preserves nested headings',()=>{
 const initial=outline('## Plan\n- Ship\n  - Test\n### Owners\n- Chad');
 const updated=outline('## Plan\n- Ship\n  - Test\n  - Release\n### Owners\n- Chad\n## Follow-up\n- Tomorrow');
 assert.equal(initial[0].id,updated[0].id);assert.equal(initial[0].children[0].id,updated[0].children[0].id);assert.equal(updated[1].title,'Follow-up');
 assert.equal(outline('```md\n## not a heading\n```').length,1);
});
test('resize bounds and manual refresh preserve a usable layout',()=>{assert.equal(resized(50,90,20,80),80);assert.equal(resized(225,-1000,150,400),150);assert.equal(resized(225,65,150,400),290);assert.equal(defaults.notes_seconds,120);assert.deepEqual(refreshRates,[30,60,120,300,0]);});
test('transcript uses confirmed merged identity while retaining turn IDs',()=>{
 const m={names:{'remote:0:A':'Chad'},speaker_aliases:{'remote:0:B':'remote:0:A'},turns:{b:{id:'b',channel:'remote',speaker:'remote:0:B',text:'hello',start:1,end:2,final_text:true,words:[{text:'hello',start:1,end:2,speaker:'remote:0:B'}]}}};
 assert.equal(speakerName(m,'remote:0:B'),'Chad');const rows=transcriptRows(m);assert.equal(rows[0].speaker,'remote:0:A');assert.equal(rows[0].turn_id,'b');
});

import {transcriptGroups,transcriptPage,paragraphSources} from '../../../frontend/src/components/cloud/transcript.ts';
import {TranscriptFollower} from '../../../frontend/src/components/cloud/follow.ts';
import {outlineDocument,expanded,emptyOutline} from '../../../frontend/src/components/cloud/outline.ts';
const row=(id,text,start,end,speaker='remote:0:A')=>({id,turn_id:id,text,start,end,speaker,channel:'remote',final:true});
test('paragraphs join provider fragments without rewriting, retain source IDs, and split speaker stretches',()=>{
 const rows=[row('a','But increasingly, what happens next',0,10),row('b','is where things change. People do their own research.',10,20),row('c','They check your website.',23,27),row('d','Yes, exactly.',28,30,'me'),row('e','And your reviews.',31,34)];
 const groups=transcriptGroups(rows);
 assert.equal(groups.length,3);assert.equal(groups[0].paragraphs.length,2);
 assert.equal(groups[0].paragraphs[0].fragments.map(f=>f.text).join(' '),'But increasingly, what happens next is where things change. People do their own research.');
 assert.deepEqual([...new Set(groups[0].paragraphs[0].fragments.map(f=>f.turn_id))],['a','b']);
 assert.equal(groups[1].speaker,'me');assert.equal(groups[2].id,'e');
 assert.equal(transcriptPage(groups,0,1).next_offset,1);assert.equal(transcriptPage(groups,2,1).next_offset,null);
 assert.throws(()=>transcriptPage(groups,-1,10));assert.throws(()=>transcriptPage(groups,0,101));
});
test('sentence splitting uses original word timing and keeps partial speakers separate',()=>{
 const r=row('a','Hello there. Next sentence.',1,5);r.words=[{text:'Hello',start:1,end:2},{text:'there.',start:2,end:3},{text:'Next',start:3.2,end:4},{text:'sentence.',start:4,end:5}];
 const groups=transcriptGroups([r,{...row('b','Still speaking',5,7,'remote:0:PENDING'),final:false}]);
 assert.equal(groups[0].paragraphs[0].fragments[1].start,3.2);assert.equal(groups[1].speaker,'remote:0:PENDING');assert.equal(groups[1].paragraphs[0].fragments[0].final,false);
});
test('following stays attached to growing text and resized panes, but respects upward user movement',()=>{
 const viewport={scrollTop:0,scrollHeight:1000,clientHeight:400};const changes=[];const follow=new TranscriptFollower(viewport,value=>changes.push(value));
 follow.set(true);assert.equal(viewport.scrollTop,600);
 viewport.scrollHeight=1100;follow.contentChanged();assert.equal(viewport.scrollTop,700);follow.scrolled();assert.deepEqual(changes,[]);
 viewport.clientHeight=350;follow.contentChanged();assert.equal(viewport.scrollTop,750);
 viewport.scrollTop=500;follow.scrolled();assert.deepEqual(changes,[false]);
 viewport.scrollHeight=1500;follow.contentChanged();assert.equal(viewport.scrollTop,500);
 follow.set(true);assert.equal(viewport.scrollTop,1150);follow.upwardIntent();assert.equal(follow.enabled,false);
});
test('outline IDs survive content updates, reordering, punctuation changes and nested additions',()=>{
 const before=outlineDocument('## Plan!\n- Ship\n  - Test\n## Owners\n- Chad',emptyOutline);
 const after=outlineDocument('## Owners\n- Chad\n## Plan\n- Ship\n  - Test\n  - Release\n- Document',emptyOutline);
 assert.deepEqual(before.nodes.map(x=>x.id).sort(),after.nodes.map(x=>x.id).sort());
 const collapsed={default_collapsed:true,overrides:{[before.nodes[0].id]:true}};
 const refreshed=outlineDocument('## Plan\n- Ship\n  - Test\n  - Release\n## New topic\n- One',collapsed);
 assert.equal(refreshed.nodes[0].expanded,true);assert.equal(refreshed.nodes.at(-1).expanded,false);assert.equal(expanded(collapsed,'new'),false);
});

test('redacting a paragraph source covers all of its sentence fragments without widening to other turns',()=>{
 const original=row('a','First sentence. Second sentence.',1,6);original.words=[{start:1,end:2},{start:2,end:3},{start:4,end:5},{start:5,end:6}];
 const paragraph=transcriptGroups([original,row('b','A different turn.',6,8)])[0].paragraphs[0];
 const sources=paragraphSources(paragraph);
 assert.equal(sources.length,2);assert.equal(sources[0].start,1);assert.equal(sources[0].end,6);assert.equal(sources[0].text,original.text);assert.equal(sources[1].turn_id,'b');
});

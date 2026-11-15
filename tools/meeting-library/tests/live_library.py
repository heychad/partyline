#!/usr/bin/env python3
"""Manual acceptance against a disposable fixture; never reads provider-key values."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time
import uuid
import wave

ROOT=Path(__file__).resolve().parents[3]
FIXTURE=ROOT/'target/partyline-library-fixture.json'
REPORT=ROOT/'target/partyline-library-live.json'
fixture=json.loads(FIXTURE.read_text())
assert fixture.get('disposable') is True
ID=fixture['id']
SERVER=ROOT/'tools/agent-control/server.py'
passed=[]


def call(tool,args,error=False):
    requests=[{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}},{'jsonrpc':'2.0','method':'notifications/initialized'},{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'partyline_'+tool,'arguments':args}}]
    result=subprocess.run([sys.executable,str(SERVER)],input=''.join(json.dumps(r)+'\n' for r in requests),capture_output=True,text=True,timeout=90,check=True)
    assert not result.stderr
    value=json.loads(result.stdout.splitlines()[-1])['result']
    if error:
        assert value.get('isError'), 'Expected rejection'
        return value
    assert not value.get('isError'), value.get('content')
    return value['structuredContent']


def current(): return call('meetings',{'action':'get','id':ID})
def edit(action,**kwargs):
    m=current()
    return call('meetings',{'action':action,'id':ID,'expected_version':m['edit_version'],**kwargs})['meeting']
def reviewed_notes():
    m=current();return call('notes',{'action':'edit','id':ID,'expected_version':m['edit_version'],'text':'# Reviewed notes\n\nShip version one on Monday. Avery owns the rollout checklist. Follow up Friday.'})
def poll(work,check,timeout=25):
    until=time.monotonic()+timeout
    while time.monotonic()<until:
        value=work()
        if check(value): return value
        time.sleep(.2)
    raise AssertionError('Operation did not reach the expected state')


def edits():
    m=current();assert m['title'].startswith('Disposable library')
    folder=Path(fixture['folder']);before={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in folder.glob('*.wav')}
    call('meetings',{'action':'open','id':ID})
    previous=m['edit_version'];m=edit('update',title='Disposable library rehearsal',participants=['Chad','Avery'])
    assert m['title']=='Disposable library rehearsal'
    call('meetings',{'action':'update','id':ID,'expected_version':previous,'title':'Stale overwrite','participants':[]},error=True)
    assert current()['title']=='Disposable library rehearsal'
    passed.append('Details persist; stale edits reject without overwriting')
    turn=next(t for t in m['turns'].values() if 'rollout' in t['text'])
    m=edit('edit_turn',turn_id=turn['id'],text='Avery owns the launch checklist.',start=turn['start'],end=turn['end'])
    assert 'launch checklist' in m['turns'][turn['id']]['text']
    m=edit('trim',start=1,end=5);m=edit('redact',start=2,end=3)
    text=call('export',{'action':'text','id':ID,'content':'transcript','format':'markdown'})['text']
    assert 'violet marmalade' not in text and 'Synthetic library check' not in text and 'launch checklist' in text
    call('export',{'action':'text','id':ID,'content':'notes','format':'markdown'},error=True)
    assert not any('violet marmalade' in h['text'] for h in call('meetings',{'action':'search','query':'violet marmalade','limit':100})['items'] if h['meeting_id']==ID)
    reviewed_notes()
    shared=call('export',{'action':'text','id':ID,'content':'both','format':'text','clipboard':True})
    assert shared['copied'] and 'violet marmalade' not in shared['text']
    passed.append('Transcript edits, trims, redactions, search, notes review, and copy/export agree')
    audio=call('export',{'action':'audio','id':ID,'channel':'both'})
    with wave.open(audio['path'],'rb') as stream:
        assert stream.getnframes()/stream.getframerate()==4
        stream.setpos(stream.getframerate());assert stream.readframes(stream.getframerate())==bytes(stream.getframerate()*2)
    assert {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in folder.glob('*.wav')}==before
    passed.append('Audio export is four seconds with the redacted second silent; originals unchanged')
    m=current();redaction=m['redactions'][-1]['id'];edit('unredact',redaction_id=redaction)
    assert 'violet marmalade' in call('export',{'action':'text','id':ID,'content':'transcript','format':'text'})['text']
    edit('undo');assert 'violet marmalade' not in call('export',{'action':'text','id':ID,'content':'transcript','format':'text'})['text']
    reviewed_notes()
    edit('trash');assert ID not in [m['id'] for m in call('meetings',{'action':'library','trash':False})['items']]
    assert ID in [m['id'] for m in call('meetings',{'action':'library','trash':True})['items']]
    call('export',{'action':'audio','id':ID,'channel':'both'},error=True)
    edit('restore');assert ID in [m['id'] for m in call('meetings',{'action':'library','trash':False})['items']]
    passed.append('Unredact, undo, Trash, rejection while trashed, and restore preserve recording data')
    call('workspace',{'action':'set','query':'Disposable library','trash':False,'panel':'none'})
    poll(lambda:call('workspace',{'action':'get'}),lambda r:r['workspace']['query']=='Disposable library')
    passed.append('Agent filters appear in the human workspace')


def playback():
    call('meetings',{'action':'open','id':ID})
    call('playback',{'action':'set','id':ID,'position':1.2,'speed':1.5,'channel':'both','playing':True})
    state=poll(lambda:call('playback',{'action':'get'}),lambda r:r.get('playback',{}).get('status')=='playing')['playback']
    assert state['speed']==1.5 and state['id']==ID
    call('playback',{'action':'set','id':ID,'playing':False,'position':3.2,'speed':2,'channel':'me'})
    state=poll(lambda:call('playback',{'action':'get'}),lambda r:r['playback']['status']=='paused' and r['playback']['speed']==2)['playback']
    assert abs(state['position']-3.2)<.1 and state['channel']=='me'
    call('playback',{'action':'set','id':ID,'position':.1},error=True)
    call('playback',{'action':'set','id':ID,'position':3.2,'channel':'remote','playing':False})
    call('playback',{'action':'close'})
    poll(lambda:call('playback',{'action':'get'}),lambda r:r['playback']['id'] is None)
    passed.append('Native playback plays, pauses, seeks, changes speed/channel, rejects trimmed-out seek, and closes')


def chat():
    if fixture.get('chat_reply_id'):
        reply=next(r for r in call('chat',{'action':'list'})['items'] if r['id']==fixture['chat_reply_id'])
    else:
        reply=call('chat',{'action':'ask','question':'What did we decide about version one, and who owns the checklist?','meeting_id':ID})['reply']
        fixture['chat_reply_id']=reply['id'];FIXTURE.write_text(json.dumps(fixture,indent=2)+'\n')
    assert all('violet marmalade' not in s['text'] for s in reply['sources'])
    result=poll(lambda:call('chat',{'action':'list'}),lambda r:any(x['id']==reply['id'] and x['status']!='running' for x in r['items']),100)
    final=next(x for x in result['items'] if x['id']==reply['id'])
    assert final['status']=='complete',final.get('error')
    assert final['sources'] and 'Monday' in final['answer'] and 'Avery' in final['answer']
    passed.append('Terra answers with source records using trimmed, non-redacted transcript excerpts')
    edit('update',title='Disposable library rehearsal',participants=['Chad','Avery'])
    old=next(x for x in call('chat',{'action':'list'})['items'] if x['id']==reply['id']);assert old['status']=='outdated' and old['answer']==''
    reply=call('chat',{'action':'ask','question':'Summarize this recording in detail.','meeting_id':ID})['reply']
    call('chat',{'action':'cancel','id':reply['id']})
    poll(lambda:call('chat',{'action':'list'}),lambda r:any(x['id']==reply['id'] and x['status']=='cancelled' for x in r['items']))
    passed.append('Changed source invalidates old answers; cancellation completes visibly')
    fixture.pop('chat_reply_id',None);FIXTURE.write_text(json.dumps(fixture,indent=2)+'\n')


def chat_cancel():
    start=time.monotonic()
    reply=call('chat',{'action':'ask','question':'What did we decide about version one?','meeting_id':ID})['reply']
    assert time.monotonic()-start<5, 'Job acceptance waited for the provider or Keychain'
    assert reply['status']=='running' and reply['phase']=='unlocking'
    call('chat',{'action':'cancel','id':reply['id']})
    poll(lambda:call('chat',{'action':'list'}),lambda r:any(x['id']==reply['id'] and x['status']=='cancelled' for x in r['items']))
    call('workspace',{'action':'set','panel':'chat'})
    passed.append('Chat returns an accepted job before Keychain approval; cancellation and other agent actions remain responsive')


def backups():
    previous=call('backup',{'action':'settings'})
    assert previous['folder'] is None, 'Do not replace an existing user destination'
    drive=Path.home()/'Library/CloudStorage/GoogleDrive-heychad@gmail.com/My Drive'
    assert drive.is_dir()
    destination=Path(fixture.get('backup_test_folder',drive/('PartyLine disposable backup test '+str(uuid.uuid4())[:8])))
    assert destination.parent==drive and destination.name.startswith('PartyLine disposable backup test ')
    destination.mkdir(exist_ok=True)
    fixture['backup_test_folder']=str(destination);FIXTURE.write_text(json.dumps(fixture,indent=2)+'\n')
    try:
        call('backup',{'action':'configure','automatic':False,'folder':'/tmp'},error=True)
        assert call('backup',{'action':'settings'})==previous
        call('backup',{'action':'configure','automatic':False,'folder':str(destination)})
        call('backup',{'action':'start','id':ID})
        def status():
            return next(x for x in call('backup',{'action':'status'})['items'] if x['meeting_id']==ID)
        state=poll(status,lambda s:s['status'] in ['copied','failed'],60)
        assert state['status']=='copied',state.get('error')
        copied=Path(state['destination'])
        assert copied.is_relative_to(destination)
        for name in ['microphone.wav','mac-audio.wav']:
            with wave.open(str(copied/name),'rb') as audio:
                assert audio.getnframes()/audio.getframerate()==4
                audio.setpos(audio.getframerate());assert audio.readframes(audio.getframerate())==bytes(audio.getframerate()*2)
        assert 'violet marmalade' not in (copied/'transcript.md').read_text()
        assert 'Synthetic library check' not in (copied/'transcript.md').read_text()
        assert not call('backup',{'action':'status'})['cloud_upload_verified']
        passed.append('Manual Drive-folder copy applies trim/redaction to both audio sources and text; upload status is not invented')
        call('backup',{'action':'configure','automatic':True,'folder':str(destination)})
        m=edit('update',title='Disposable library rehearsal',participants=['Chad','Avery'])
        state=poll(status,lambda s:s['status']=='copied' and s['edit_version']==m['edit_version'],60)
        assert Path(state['destination']).is_dir()
        call('backup',{'action':'start','id':ID})
        poll(status,lambda s:s['status']=='copied',60)
        passed.append('Finished-recording edits trigger automatic backup; repeat backup succeeds')
    finally:
        call('backup',{'action':'configure','automatic':previous['automatic'],'folder':previous['folder'] or ''})
        assert call('backup',{'action':'settings'})==previous
    passed.append('Invalid folder rejected; destination reset to unselected with automatic backup enabled')
    print('Disposable backup folder:',destination)


def clients():
    status=call('clients',{'action':'status'})
    assert status['transport']=='stdio' and status['app_required']
    for client in ['codex','claude-code','claude-desktop']:
        result=call('clients',{'action':'setup','client':client})
        assert result['ok'] and result['restart_required']
        server=Path(result['server']);assert server.is_file() and server.with_name('tools.json').is_file()
        requests=[{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25'}},{'jsonrpc':'2.0','method':'notifications/initialized'},{'jsonrpc':'2.0','id':2,'method':'tools/list','params':{}},{'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'partyline_backup','arguments':{'action':'settings'}}}]
        wire=subprocess.run([sys.executable,str(server)],input=''.join(json.dumps(r)+'\n' for r in requests),text=True,capture_output=True,check=True,timeout=15)
        replies=[json.loads(line) for line in wire.stdout.splitlines()]
        assert len(replies[1]['result']['tools'])==13
        assert replies[2]['result']['structuredContent']['folder'] is None
        passed.append(client+' configured; packaged stdio bridge discovers 13 tools and reaches the live app')


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('phase',choices=['edits','playback','chat','chat_cancel','backups','clients']);phase=parser.parse_args().phase
    try:
        globals()[phase]()
        data=json.loads(REPORT.read_text()) if REPORT.exists() else {'fixture':ID,'phases':{}}
        data['phases'][phase]={'passed':passed};REPORT.write_text(json.dumps(data,indent=2)+'\n')
        print(json.dumps({'phase':phase,'passed':passed}))
    except Exception as error:
        print('Live acceptance failed:',str(error),file=sys.stderr);raise SystemExit(1)

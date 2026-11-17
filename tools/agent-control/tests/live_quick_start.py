"""Manual quick-start acceptance. Pass the ID started by the test operator.
edit ID updates that active recording, then opens its details editor.
finish ID verifies the human edit and stops only that same recording.
Uses saved provider connections; audio is sent to the configured providers.
"""
import json
import sys
import time
import wave
from pathlib import Path
from live_workspace import call, until, ROOT

REPORT=ROOT/'target/partyline-044-acceptance.json'

def run(mode, identifier):
    status=call('status',{'action':'get'})
    assert status['version']=='0.4.4' and status['agent_access']=='control'
    if mode=='edit':
        assert status['current'] and status['current']['id']==identifier
        until(lambda:call('status',{'action':'get'}),lambda s:s['current'] and s['current']['id']==identifier and s['current']['status']=='recording')
        m=call('meetings',{'action':'get','id':identifier})
        assert m['title'].startswith('Meeting · ') and m['edit_version']==0
        result=call('meetings',{'action':'update','id':identifier,'expected_version':0,'title':'Disposable quick-start acceptance','participants':['Avery','Jordan']})['meeting']
        assert result['status']=='recording' and result['participants']==['Avery','Jordan']
        assert result['names']==m['names'] and result['undo_version'] is None
        call('meetings',{'action':'update','id':identifier,'expected_version':0,'title':'Stale edit','participants':[]},error=True)
        call('meetings',{'action':'update','id':identifier,'expected_version':1,'title':' ','participants':[]},error=True)
        call('workspace',{'action':'set','panel':'details'})
        until(lambda:call('workspace',{'action':'get'}),lambda v:v['workspace']['panel']=='details')
        REPORT.write_text(json.dumps({'id':identifier,'disposable':True,'version':status['version'],'default_title':m['title'],'live_agent_details':True,'stale_and_invalid_rejected':True,'no_automatic_identity_assignment':True,'editor_opened_during_capture':True},indent=2))
        print('Live agent edit passed. Change the title to Disposable quick-start checked in the open editor, then run finish.')
    elif mode=='finish':
        report=json.loads(REPORT.read_text());assert report['id']==identifier and report['disposable']
        assert status['current'] and status['current']['id']==identifier
        try:
            m=call('meetings',{'action':'get','id':identifier})
            assert m['title']=='Disposable quick-start checked' and m['edit_version']==2
            assert m['participants']==['Avery','Jordan'] and m['status']=='recording'
            report['live_human_edit']=True
        finally:
            call('recording',{'action':'stop','expected_meeting_id':identifier})
        for _ in range(120):
            if call('status',{'action':'get'})['current'] is None:break
            time.sleep(.25)
        else:raise AssertionError('Recording is still finalizing; inspect status before retrying.')
        m=call('meetings',{'action':'get','id':identifier});assert m['status']=='complete'
        assert m['duration']>5 and m['capture']['durable_seconds']>5
        folder=Path.home()/'Library/Application Support/com.heychad.partyline/cloud-meetings'/identifier
        durations=[]
        for name in ['microphone.wav','mac-audio.wav']:
            with wave.open(str(folder/name)) as audio:durations.append(audio.getnframes()/audio.getframerate())
        assert abs(durations[0]-durations[1])<.01 and min(durations)>5
        report.update(duration=m['duration'],audio_seconds=durations,errors=m['errors'],title=m['title'],participants=m['participants'],capture=m['capture'])
        REPORT.write_text(json.dumps(report,indent=2));print('Live human/agent details edits passed; recording completed with matching local audio tracks.')
    else:raise ValueError('Use edit or finish')

if __name__=='__main__':run(sys.argv[1],sys.argv[2])

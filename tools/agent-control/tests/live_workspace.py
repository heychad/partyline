"""Manual 0.4.3 acceptance against an idle app and explicitly disposable fixture.
Run: python3 tools/agent-control/tests/live_workspace.py exercise|restart
Uses the existing target/partyline-042-fixture.json; never creates a recording.
"""
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REPORT = ROOT / 'target/partyline-043-acceptance.json'
FIXTURE = ROOT / 'target/partyline-042-fixture.json'

def call(name, arguments, error=False):
    messages = [{'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocolVersion': '2025-11-25'}}, {'jsonrpc': '2.0', 'method': 'notifications/initialized'}, {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/call', 'params': {'name': 'partyline_' + name, 'arguments': arguments}}]
    result = subprocess.run([sys.executable, str(ROOT / 'tools/agent-control/server.py')], input=''.join(json.dumps(x)+'\n' for x in messages), text=True, capture_output=True, timeout=30, check=True)
    response = json.loads(result.stdout.splitlines()[-1])['result']
    assert bool(response.get('isError')) == error, response.get('content')
    return response if error else response['structuredContent']

def until(read, predicate):
    for _ in range(50):
        value = read()
        if predicate(value): return value
        time.sleep(.1)
    raise AssertionError('Expected visible state did not arrive')

def run(mode):
    status = call('status', {'action': 'get'})
    assert status['current'] is None and status['version'] == '0.4.3'
    fixture = json.loads(FIXTURE.read_text())
    assert fixture['disposable'] is True
    identifier = fixture['id']
    meeting = call('meetings', {'action': 'get', 'id': identifier})
    assert meeting['title'].startswith('Disposable ')
    call('meetings', {'action': 'open', 'id': identifier})
    def outline(): return call('workspace', {'action': 'outline', 'id': identifier})
    def change(change): return call('workspace', {'action': 'outline_set', 'id': identifier, 'change': change})
    until(outline, lambda v: v['ready'])
    if mode == 'restart':
        report = json.loads(REPORT.read_text())
        assert status['agent_access'] == 'control'
        assert outline()['state']['default_collapsed'] is True
        assert all(not x['expanded'] for x in outline()['nodes'])
        assert call('workspace', {'action': 'preferences'}) == report['preferences_before']
        report['restart_persistence'] = True
        change({'operation': 'expand_all'})
        m = call('meetings', {'action': 'get', 'id': identifier})
        call('notes', {'action': 'edit', 'id': identifier, 'expected_version': m['edit_version'], 'text': report['fixture_notes_before']})
        REPORT.write_text(json.dumps(report, indent=2))
        print('Restart preserved Control, preferences, and collapsed outline. Fixture notes restored.')
        return
    preferences = call('workspace', {'action': 'preferences'})
    report = {'version': status['version'], 'fixture_id': identifier, 'preferences_before': preferences, 'fixture_notes_before': meeting['notes']}
    REPORT.write_text(json.dumps(report, indent=2))
    try:
        change({'operation': 'collapse_all'})
        collapsed = until(outline, lambda v: all(not n['expanded'] for n in v['nodes']))
        node = collapsed['nodes'][0]['id']
        change({'operation': 'set', 'node_id': node, 'expanded': True})
        until(outline, lambda v: next(n for n in v['nodes'] if n['id'] == node)['expanded'])
        call('notes', {'action': 'edit', 'id': identifier, 'expected_version': meeting['edit_version'], 'text': meeting['notes']+'\n## Added during refresh\n- New parent\n  - A nested item\n'})
        updated = until(outline, lambda v: any(n['label'] == 'Added during refresh' for n in v['nodes']))
        assert next(n for n in updated['nodes'] if n['id'] == node)['expanded']
        assert not next(n for n in updated['nodes'] if n['label'] == 'Added during refresh')['expanded']
        call('workspace', {'action': 'outline_set', 'id': identifier, 'change': {'operation': 'set', 'node_id': 'missing', 'expanded': True}}, error=True)
        transcript = call('workspace', {'action': 'transcript', 'id': identifier, 'offset': 0, 'limit': 1})
        assert len(transcript['items']) == 1
        assert all(f['turn_id'] in meeting['turns'] for p in transcript['items'][0]['paragraphs'] for f in p['fragments'])
        for panel in ['view', 'speakers', 'view', 'chat', 'none']:
            call('workspace', {'action': 'set', 'panel': panel})
            until(lambda: call('workspace', {'action': 'get'}), lambda v: v['workspace']['panel'] == panel)
        patch = {'sidebar_width': 245, 'transcript_percent': 58, 'chat_width': 320, 'notes_seconds': 120, 'sidebar_visible': False}
        call('workspace', {'action': 'configure', 'patch': patch})
        until(lambda: call('workspace', {'action': 'get'}), lambda v: all(v['workspace']['preferences'][k] == value for k, value in patch.items()))
        change({'operation': 'collapse_all'})
        report['live_controls'] = {'outline_updates_and_new_nodes': True, 'stale_node_rejected': True, 'transcript_source_ranges': True, 'panels_and_preferences': True}
    finally:
        call('workspace', {'action': 'configure', 'patch': preferences})
        REPORT.write_text(json.dumps(report, indent=2))
    print('Live workspace controls passed. Collapsed outline left for restart check.')

if __name__ == '__main__':
    run(sys.argv[1])

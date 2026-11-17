"""Manual calendar acceptance after human consent; uses only the disposable fixture.

Requires two connected accounts and an idle app. Temporarily deselects one
account's calendars, then restores them. Never reads tokens or starts capture.
"""
import hashlib
import json
import time
from pathlib import Path
from live_workspace import call, ROOT


def settled():
    for _ in range(120):
        state = call('calendar', {'action': 'status'})
        if state['phase'] == 'idle':
            assert not state['error'] and not state['account_errors']
            return state
        time.sleep(.5)
    raise AssertionError('Calendar refresh did not finish within 60 seconds')


def run():
    assert call('status', {'action': 'get'})['current'] is None
    state = settled()
    assert len(state['accounts']) >= 2
    fixture = json.loads((ROOT / 'target/partyline-042-fixture.json').read_text())
    assert fixture['disposable'] is True
    identifier = fixture['id']
    original = call('meetings', {'action': 'get', 'id': identifier})
    assert original['title'].startswith('Disposable ') and not original.get('calendar_event')
    audio = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
             for p in Path(fixture['folder']).glob('*.wav')}
    report = {'accounts': len(state['accounts']), 'events': len(state['events'])}
    account = state['accounts'][0]
    others = {e['key'] for e in state['events'] if e['account_id'] != account['id']}
    try:
        call('calendar', {'action': 'select', 'account_id': account['id'], 'calendar_ids': []})
        empty = settled()
        assert not any(e['account_id'] == account['id'] for e in empty['events'])
        assert others == {e['key'] for e in empty['events']}
        report['calendar_selection_isolated'] = True
    finally:
        call('calendar', {'action': 'select', 'account_id': account['id'], 'calendar_ids': account['selected']})
        state = settled()
    event = next(e for e in state['events'] if e['account_id'] == account['id'])
    try:
        linked = call('calendar', {'action': 'link', 'id': identifier,
            'expected_version': original['edit_version'], 'event_key': event['key'], 'use_details': False})['meeting']
        assert linked['calendar_event']['key'] == event['key']
        assert linked['title'] == original['title'] and linked['participants'] == original['participants']
        call('calendar', {'action': 'unlink', 'id': identifier,
            'expected_version': original['edit_version']}, error=True)
        detailed = call('calendar', {'action': 'link', 'id': identifier,
            'expected_version': linked['edit_version'], 'event_key': event['key'], 'use_details': True})['meeting']
        assert detailed['title'] == event['title'] and detailed['participants'] == event['participants']
        unlinked = call('calendar', {'action': 'unlink', 'id': identifier,
            'expected_version': detailed['edit_version']})['meeting']
        assert unlinked['calendar_event'] is None and unlinked['title'] == event['title']
        report['link_details_unlink_and_stale_guard'] = True
    finally:
        current = call('meetings', {'action': 'get', 'id': identifier})
        if current.get('calendar_event'):
            current = call('calendar', {'action': 'unlink', 'id': identifier,
                'expected_version': current['edit_version']})['meeting']
        call('meetings', {'action': 'update', 'id': identifier,
            'expected_version': current['edit_version'], 'title': original['title'],
            'participants': original['participants']})
    assert audio == {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                     for p in Path(fixture['folder']).glob('*.wav')}
    report['fixture_details_restored_audio_unchanged'] = True
    (ROOT / 'target/partyline-045-connected-acceptance.json').write_text(json.dumps(report, indent=2))
    print('Multi-account selection, event attachment, stale guards, and fixture restoration passed.')


if __name__ == '__main__':
    run()

"""Manual native acceptance for calendar controls before Google consent.
Does not start a recording, authorize Google, or write provider credentials.
Retains the pre-test detection setting and never changes agent access.
"""
import json
import time
from live_workspace import call, until, ROOT


def run():
    status = call('status', {'action': 'get'})
    assert status['version'] == '0.4.5' and status['agent_access'] == 'control'
    assert status['current'] is None
    original = call('calendar', {'action': 'status'})
    report = {'version': status['version'], 'no_active_recording': True,
              'configured': original['configured'], 'account_count': len(original['accounts'])}
    call('workspace', {'action': 'set', 'panel': 'calendar'})
    until(lambda: call('workspace', {'action': 'get'}), lambda s: s['workspace']['panel'] == 'calendar')
    report['agent_opened_visible_panel'] = True
    call('calendar', {'action': 'select', 'account_id': 'does-not-exist', 'calendar_ids': ['primary']}, error=True)
    call('calendar', {'action': 'disconnect', 'account_id': 'does-not-exist'}, error=True)
    call('calendar', {'action': 'configure_client', 'client_id': 'invalid', 'client_secret': 'INVALID-DO-NOT-SAVE'}, error=True)
    call('calendar', {'action': 'dismiss', 'signal_id': 'does-not-exist'}, error=True)
    call('calendar', {'action': 'cancel'}, error=True)
    report['invalid_actions_rejected'] = True
    try:
        call('calendar', {'action': 'detection_configure', 'enabled': True})
        observed = until(lambda: call('calendar', {'action': 'status'}), lambda s: bool(s['detection']['last_scan']))
        assert observed['detection']['enabled'] and observed['detection']['error'] is None
        report['native_detection_checked'] = True
        report['signal_count'] = len(observed['detection']['signals'])
        report['last_scan'] = observed['detection']['last_scan']
    finally:
        call('calendar', {'action': 'detection_configure', 'enabled': original['detection']['enabled']})
    final = call('calendar', {'action': 'status'})
    assert final['configured'] == original['configured'] and final['accounts'] == original['accounts']
    assert call('status', {'action': 'get'})['current'] is None
    report['settings_restored'] = True
    report['no_recording_started'] = True
    (ROOT / 'target/partyline-045-calendar-acceptance.json').write_text(json.dumps(report, indent=2))
    print('Calendar panel, input validation, native detector, and restored settings passed.')


if __name__ == '__main__':
    run()

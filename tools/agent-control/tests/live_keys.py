"""Manual native acceptance test. Only connects to PartyLine's isolated key-test profile.

Run seed, restart that app, run verify, then remove. Never calls a provider.
Requests go through MCP stdio; key inputs are never printed or written to logs.
"""
import argparse
import json
from pathlib import Path
import secrets
import subprocess
import sys

SOCKET = Path.home() / 'Library/Application Support/com.heychad.partyline.key-tests/agent-control/control.sock'
SERVER = Path(__file__).resolve().parents[1] / 'server.py'


class Check:
    def __init__(self):
        self.inputs = []
        self.passed = []

    def key(self):
        value = 'partyline-test-' + secrets.token_hex(24)
        self.inputs.append(value)
        return value

    def call(self, tool, arguments, error=False):
        messages = [
            dict(jsonrpc='2.0', id=1, method='initialize', params={'protocolVersion': '2025-11-25'}),
            dict(jsonrpc='2.0', method='notifications/initialized'),
            dict(jsonrpc='2.0', id=2, method='tools/call', params={'name': 'partyline_' + tool, 'arguments': arguments}),
        ]
        process = subprocess.run([sys.executable, str(SERVER), '--socket', str(SOCKET)],
                                 input=''.join(json.dumps(m) + '\n' for m in messages),
                                 text=True, capture_output=True, timeout=80, check=True)
        assert not process.stderr, 'Unexpected server diagnostics'
        assert all(value not in process.stdout for value in self.inputs), 'A test input escaped in a response'
        replies = [json.loads(line) for line in process.stdout.splitlines()]
        assert len(replies) == 2 and replies[-1]['id'] == 2, 'Invalid MCP response'
        result = replies[-1]['result']
        assert result['isError'] == error, 'Unexpected tool outcome'
        if error:
            return None
        value = result['structuredContent']
        if tool == 'connections':
            assert set(value) == {'assemblyai', 'openai', 'ok'}, 'Unexpected connection result field'
            assert all(isinstance(item, bool) for item in value.values()), 'Connection values must be booleans'
        return value

    def flags(self, assemblyai, openai):
        value = self.call('connections', {'action': 'status'})
        assert value == dict(assemblyai=assemblyai, openai=openai, ok=True), 'Unexpected stored key state'

    def run(self, phase):
        state = self.call('status', {'action': 'get'})
        assert state['agent_access'] == 'control' and state['current'] is None, 'Use Control in an idle test profile'
        if phase == 'seed':
            self.flags(False, False)
            self.call('connections', {'action': 'save', 'assemblyai': 'invalid whitespace'}, error=True)
            self.flags(False, False)
            self.passed.append('Invalid input leaves both keys absent')
            self.call('connections', {'action': 'save', 'assemblyai': self.key()})
            self.flags(True, False)
            self.call('connections', {'action': 'save', 'openai': self.key()})
            self.flags(True, True)
            self.passed.append('Each provider saves independently through the real native Keychain command')
            self.call('connections', {'action': 'save', 'assemblyai': self.key()})
            self.flags(True, True)
            self.passed.append('Replacing one test key preserves the other provider')
        elif phase == 'verify':
            self.flags(True, True)
            self.passed.append('Saved Control and both keys survive a full app restart')
        elif phase == 'remove':
            self.flags(True, True)
            self.call('connections', {'action': 'remove', 'provider': 'assemblyai'})
            self.flags(False, True)
            self.call('connections', {'action': 'remove', 'provider': 'assemblyai'}, error=True)
            self.flags(False, True)
            self.passed.append('Removing a key leaves the other provider intact; repeated removal reports an error')
            self.call('connections', {'action': 'save', 'assemblyai': self.key()})
            self.flags(True, True)
            self.call('connections', {'action': 'remove', 'provider': 'openai'})
            self.flags(True, False)
            self.call('connections', {'action': 'remove', 'provider': 'assemblyai'})
            self.flags(False, False)
            self.passed.append('Both provider removal paths work; all test keys cleaned up')
        self.call('status', {'action': 'activity'})
        self.passed.append('Results and activity contain no key inputs')
        print(json.dumps({'profile': 'com.heychad.partyline.key-tests', 'phase': phase, 'passed': self.passed}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=['seed', 'verify', 'remove'])
    args = parser.parse_args()
    try:
        Check().run(args.phase)
    except Exception as exc:
        # Do not print exception values or requests: they might contain an input.
        print('Isolated key acceptance failed (' + type(exc).__name__ + '). Inspect app state before retrying.', file=sys.stderr)
        raise SystemExit(1) from None

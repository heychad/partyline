import io
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import catalog
import server


def request(method, params=None, identifier=1):
    return dict(jsonrpc='2.0', id=identifier, method=method, params=params or {})


def ready(transport=server.connect):
    bridge = server.Server(transport=transport)
    bridge.handle(request('initialize', dict(protocolVersion='2025-11-25')))
    bridge.handle(dict(jsonrpc='2.0', method='notifications/initialized'))
    return bridge


class ProtocolTests(unittest.TestCase):
    def test_catalog_is_current_and_covers_every_cloud_command(self):
        self.assertEqual(catalog.TOOLS, json.loads(catalog.CATALOG.read_text()))
        self.assertEqual(len(catalog.TOOLS), 13)
        # Actual native entry points must be visible in the adapter; no generic invoke bypass.
        import re
        root = catalog.ROOT / 'frontend/src-tauri/src'
        commands = []
        for name in ('cloud/mod.rs', 'cloud/keys.rs'):
            commands += re.findall(r'#\[tauri::command\]\s*pub (?:async )?fn (\w+)', (root / name).read_text())
        adapter = (root / 'agent_control/native.rs').read_text()
        for command in commands:
            self.assertIn(command + '(', adapter, command)
        self.assertNotIn('keys::read(', adapter)

    def test_lifecycle_versions_and_tool_discovery_without_app(self):
        bridge = server.Server()
        self.assertIn('error', bridge.handle(request('tools/list')))
        result = bridge.handle(request('initialize', dict(protocolVersion='future-version')))
        self.assertEqual(result['result']['protocolVersion'], server.VERSIONS[0])
        self.assertIsNone(bridge.handle(dict(jsonrpc='2.0', method='notifications/initialized')))
        self.assertEqual(len(bridge.handle(request('tools/list'))['result']['tools']), 13)
        self.assertIn('error', bridge.handle(request('initialize', dict(protocolVersion=server.VERSIONS[0]))))
        self.assertIn('error', bridge.handle(request('tools/list', dict(cursor='unknown'))))

    def test_calls_return_structured_and_text_content(self):
        calls = []
        def transport(path, value):
            calls.append(value)
            return dict(status='paused')
        result = ready(transport).handle(request('tools/call', dict(name='partyline_status', arguments={'action': 'get'})))['result']
        self.assertFalse(result['isError'])
        self.assertEqual(result['structuredContent'], json.loads(result['content'][0]['text']))
        self.assertEqual(calls, [dict(tool='partyline_status', arguments={'action': 'get'})])

    def test_malformed_input_does_not_echo_values(self):
        secret = 'sk-test-secret-value'
        bridge = ready()
        for value in ([], request(secret), request('tools/call', dict(name=secret)), request('tools/call', dict(name='partyline_status', arguments=secret))):
            result = bridge.handle(value)
            self.assertIn('error', result)
            self.assertNotIn(secret, json.dumps(result))
        output = io.BytesIO()
        server.serve(io.BytesIO(b'{"broken":sk-test-secret-value}\n'), output, bridge)
        self.assertNotIn(secret.encode(), output.getvalue())

    def test_notifications_cannot_execute_actions(self):
        calls = []
        bridge = ready(lambda *args: calls.append(args))
        self.assertIsNone(bridge.handle(dict(jsonrpc='2.0', method='tools/call', params={'name': 'partyline_recording', 'arguments': {'action': 'start'}})))
        self.assertEqual(calls, [])

    def test_failed_call_is_tool_error_and_is_not_retried(self):
        calls = []
        def unavailable(*args):
            calls.append(args)
            raise RuntimeError('Outcome unknown. Inspect status.')
        result = ready(unavailable).handle(request('tools/call', dict(name='partyline_recording', arguments={'action': 'start'})))['result']
        self.assertTrue(result['isError'])
        self.assertEqual(len(calls), 1)

    def test_bounded_lines_recover_for_next_request(self):
        output = io.BytesIO()
        payload = b'x' * (server.MAX_INPUT + 2) + b'\n' + json.dumps(request('ping')).encode() + b'\n'
        server.serve(io.BytesIO(payload), output, ready())
        results = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual(len(results), 2)
        self.assertIn('error', results[0])
        self.assertEqual(results[1]['result'], {})

    def test_real_stdio_process_outputs_only_protocol(self):
        messages = [request('initialize', dict(protocolVersion='2024-11-05')),
                    dict(jsonrpc='2.0', method='notifications/initialized'), request('tools/list', identifier=2)]
        result = subprocess.run([sys.executable, str(Path(server.__file__))],
                                input=''.join(json.dumps(m) + '\n' for m in messages),
                                capture_output=True, text=True, check=True)
        self.assertEqual(result.stderr, '')
        responses = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(len(responses), 2)
        self.assertEqual(len(responses[1]['result']['tools']), 13)

    def test_local_ui_commands_are_registered_and_permissioned(self):
        root = catalog.ROOT / 'frontend/src-tauri'
        config = json.loads((root / 'tauri.conf.json').read_text())
        permissions = config['app']['security']['capabilities'][0]['permissions']
        build = (root / 'build.rs').read_text()
        for command in ('permissions', 'set-permissions', 'ui-result', 'ui-state'):
            self.assertIn('agent-control:allow-' + command, permissions)
            self.assertIn('"' + command.replace('-', '_') + '"', build)
        self.assertIn('.plugin(agent_control::init())', (root / 'src/lib.rs').read_text())
        view = (catalog.ROOT / 'frontend/src/components/cloud/CloudWorkspace.tsx').read_text()
        self.assertIn('useCloudAgentBridge({', view)
        self.assertIn('<AgentAccess />', view)


class TransportTests(unittest.TestCase):
    def test_real_private_socket_roundtrip_and_permissions(self):
        with tempfile.TemporaryDirectory(prefix='pl-') as folder:
            os.chmod(folder, 0o700)
            path = Path(folder) / 'c.sock'
            listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            listener.bind(str(path)); listener.listen(1); os.chmod(path, 0o600)
            received = []
            def host():
                with listener.accept()[0] as peer:
                    with peer.makefile('rb') as reader:
                        received.append(json.loads(reader.readline()))
                    peer.sendall(b'{"ok":true,"result":{"sample_age_ms":120,"fresh":true}}\n')
            worker = threading.Thread(target=host); worker.start()
            value = server.connect(path, dict(tool='partyline_audio', arguments={'action': 'diagnostics'}))
            worker.join(timeout=2); listener.close()
            self.assertEqual(value['sample_age_ms'], 120)
            self.assertEqual(received[0]['arguments']['action'], 'diagnostics')
            os.chmod(path, 0o666)
            with self.assertRaisesRegex(RuntimeError, 'not private'):
                server.connect(path, {})

    def test_symlink_endpoint_is_rejected_without_connecting(self):
        with tempfile.TemporaryDirectory(prefix='pl-') as folder:
            path = Path(folder) / 'link'
            path.symlink_to(Path(folder) / 'target')
            with self.assertRaisesRegex(RuntimeError, 'not private'):
                server.connect(path, {})


if __name__ == '__main__':
    unittest.main()

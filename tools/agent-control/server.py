#!/usr/bin/env python3
"""MCP stdio adapter for PartyLine's permissioned Unix socket. Standard library only."""
import argparse
import json
import os
from pathlib import Path
import socket
import stat
import sys

CATALOG = Path(__file__).resolve().with_name('tools.json')
if not CATALOG.is_file():
    CATALOG = Path(__file__).resolve().parents[2] / 'frontend/src-tauri/src/agent_control/tools.json'
DEFAULT_SOCKET = Path.home() / 'Library/Application Support/com.heychad.partyline/agent-control/control.sock'
MAX_INPUT = 512 * 1024
MAX_OUTPUT = 16 * 1024 * 1024
VERSIONS = ('2025-11-25', '2025-06-18', '2025-03-26', '2024-11-05')


def decode(value):
    def reject_constant(_):
        raise ValueError('Non-JSON number')
    return json.loads(value, parse_constant=reject_constant)


def rpc_error(identifier, code, message):
    return dict(jsonrpc='2.0', id=identifier, error=dict(code=code, message=message))


def rpc_result(identifier, result):
    return dict(jsonrpc='2.0', id=identifier, result=result)


def connect(path, request):
    """No retries: a timed-out write may already have changed the recording."""
    path = Path(path)
    try:
        directory, endpoint = path.parent.lstat(), path.lstat()
    except OSError:
        raise RuntimeError('PartyLine agent control is unavailable. Open a build with agent control enabled.') from None
    if (not stat.S_ISDIR(directory.st_mode) or not stat.S_ISSOCK(endpoint.st_mode)
            or directory.st_uid != os.geteuid() or endpoint.st_uid != os.geteuid()
            or directory.st_mode & 0o077 or endpoint.st_mode & 0o077):
        raise RuntimeError('The PartyLine socket is not private to this macOS account.')
    payload = (json.dumps(request, separators=(',', ':'), ensure_ascii=False) + '\n').encode()
    if len(payload) > MAX_INPUT:
        raise RuntimeError('Tool arguments are too large.')
    sent = False
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as stream:
            stream.settimeout(5)
            stream.connect(str(path))
            # A partial send can still leave the server with a complete request.
            sent = True
            stream.sendall(payload)
            stream.settimeout(65)
            with stream.makefile('rb') as reader:
                response = reader.readline(MAX_OUTPUT + 2)
            if len(response) > MAX_OUTPUT + 1 or not response.endswith(b'\n'):
                raise ValueError('Invalid response frame')
            result = decode(response)
            if not isinstance(result, dict) or not isinstance(result.get('ok'), bool):
                raise ValueError('Invalid response')
            if not result['ok']:
                # Socket errors originate in the app, never in request arguments.
                raise RuntimeError(result.get('error') if isinstance(result.get('error'), str) else 'PartyLine could not perform this action.')
            if not isinstance(result.get('result'), dict):
                raise ValueError('Invalid tool result')
            return result['result']
    except (OSError, ValueError, UnicodeError):
        if sent:
            raise RuntimeError('PartyLine did not return a complete result. The action may have run. Read app status before retrying.') from None
        raise RuntimeError('Could not connect to PartyLine. Open the app and enable agent access.') from None


class Server:
    def __init__(self, path=DEFAULT_SOCKET, transport=connect):
        self.path, self.transport = path, transport
        self.initialized = False
        self.ready = False
        self.tools = decode(CATALOG.read_text())

    def handle(self, request):
        if not isinstance(request, dict):
            return rpc_error(None, -32600, 'Invalid request.')
        identifier = request.get('id')
        if (request.get('jsonrpc') != '2.0' or not isinstance(request.get('method'), str)
                or ('id' in request and (isinstance(identifier, bool) or not isinstance(identifier, (str, int))))):
            return rpc_error(None, -32600, 'Invalid request.')
        method = request['method']
        params = request.get('params', {})
        if not isinstance(params, dict):
            return None if 'id' not in request else rpc_error(identifier, -32602, 'Invalid parameters.')
        if 'id' not in request:
            if method == 'notifications/initialized' and self.initialized:
                self.ready = True
            return None
        if method == 'initialize':
            if self.initialized:
                return rpc_error(identifier, -32600, 'Already initialized.')
            version = params.get('protocolVersion')
            if not isinstance(version, str):
                return rpc_error(identifier, -32602, 'Missing protocol version.')
            self.initialized = True
            return rpc_result(identifier, dict(
                protocolVersion=version if version in VERSIONS else VERSIONS[0],
                capabilities={'tools': {}}, serverInfo={'name': 'partyline', 'version': '0.1.0'},
                instructions='Use PartyLine for cloud meetings. Enable read or control access in the app. Never infer speaker identities from participant suggestions. After a transport failure, inspect status before retrying a mutation.'))
        if method == 'ping':
            return rpc_result(identifier, {})
        if not self.ready:
            return rpc_error(identifier, -32000, 'Initialize the MCP session first.')
        if method == 'tools/list':
            if params.get('cursor'):
                return rpc_error(identifier, -32602, 'Invalid tools cursor.')
            return rpc_result(identifier, {'tools': self.tools})
        if method == 'tools/call':
            name, arguments = params.get('name'), params.get('arguments', {})
            if not isinstance(name, str) or not any(tool['name'] == name for tool in self.tools):
                return rpc_error(identifier, -32602, 'Unknown tool.')
            if not isinstance(arguments, dict):
                return rpc_error(identifier, -32602, 'Tool arguments must be an object.')
            try:
                value = self.transport(self.path, dict(tool=name, arguments=arguments))
                result = dict(content=[dict(type='text', text=json.dumps(value, ensure_ascii=False))], structuredContent=value, isError=False)
            except RuntimeError as error:
                result = dict(content=[dict(type='text', text=str(error))], isError=True)
            return rpc_result(identifier, result)
        return rpc_error(identifier, -32601, 'Method not found.')


def serve(source, destination, server):
    while True:
        line = source.readline(MAX_INPUT + 1)
        if not line:
            break
        if len(line) > MAX_INPUT or not line.endswith(b'\n'):
            while line and not line.endswith(b'\n'):
                line = source.readline(MAX_INPUT + 1)
            response = rpc_error(None, -32700, 'Request is too large or incomplete.')
        else:
            try:
                response = server.handle(decode(line))
            except (ValueError, UnicodeError, RecursionError):
                response = rpc_error(None, -32700, 'Invalid JSON.')
        if response is not None:
            destination.write((json.dumps(response, separators=(',', ':'), ensure_ascii=False) + '\n').encode())
            destination.flush()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--socket', type=Path, default=DEFAULT_SOCKET)
    options = parser.parse_args()
    try:
        serve(sys.stdin.buffer, sys.stdout.buffer, Server(options.socket))
    except (BrokenPipeError, KeyboardInterrupt):
        pass

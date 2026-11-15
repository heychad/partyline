#!/usr/bin/env python3
"""Install PartyLine's local MCP bridge in a named client. Never prints client configuration."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def merge_desktop(path, server, python):
    old = path.read_bytes() if path.exists() else None
    data = json.loads(old) if old else {}
    if not isinstance(data, dict) or not isinstance(data.get('mcpServers', {}), dict):
        raise ValueError('Invalid Claude configuration')
    config = {'command': python, 'args': [str(server)]}
    if data.get('mcpServers', {}).get('partyline') == config:
        return False
    data.setdefault('mcpServers', {})['partyline'] = config
    path.parent.mkdir(parents=True, exist_ok=True)
    if old is not None:
        backup = path.with_name(path.name + '.partyline-backup')
        # Keep the first pre-install copy; avoid repeatedly copying unrelated secrets.
        if not backup.exists():
            fd = os.open(backup, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, 'wb') as output:
                output.write(old)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as output:
        temporary = Path(output.name)
        output.write((json.dumps(data, indent=2) + '\n').encode())
    try:
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)
    return True


def command(name):
    candidates = [shutil.which(name), str(Path.home() / '.local/bin' / name), '/opt/homebrew/bin/' + name, '/usr/local/bin/' + name]
    return next((p for p in candidates if p and os.path.isfile(p) and os.access(p, os.X_OK)), None)


def claude_code_configured(path, server, python):
    if not path.exists():
        return False
    data = json.loads(path.read_text())
    config = data.get('mcpServers', {}).get('partyline', {})
    return (config.get('type', 'stdio') == 'stdio' and config.get('command') == python
            and config.get('args') == [str(server)] and not config.get('env'))


def install(client, server):
    server = server.resolve(strict=True)
    if client == 'claude-desktop':
        merge_desktop(Path.home() / 'Library/Application Support/Claude/claude_desktop_config.json', server, sys.executable)
    elif client != 'claude-code' or not claude_code_configured(Path.home() / '.claude.json', server, sys.executable):
        binary = command('codex' if client == 'codex' else 'claude')
        if not binary:
            raise RuntimeError('Client CLI is not installed')
        args = [binary, 'mcp', 'add']
        if client == 'claude-code':
            args += ['--scope', 'user', '--transport', 'stdio']
        args += ['partyline', '--', sys.executable, str(server)]
        result = subprocess.run(args, capture_output=True, text=True, timeout=30)
        if result.returncode:
            # Do not echo stdout/stderr: another client's configuration can contain secrets.
            raise RuntimeError('Client refused MCP setup; an existing partyline entry may need review')
    return {'ok': True, 'client': client, 'server': str(server), 'restart_required': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--client', choices=['codex', 'claude-code', 'claude-desktop'], required=True)
    parser.add_argument('--server', type=Path, default=Path(__file__).with_name('server.py'))
    args = parser.parse_args()
    try:
        print(json.dumps(install(args.client, args.server)))
    except Exception:
        print('Could not configure this client. Check that it is installed and review any existing partyline MCP entry.', file=sys.stderr)
        raise SystemExit(1) from None

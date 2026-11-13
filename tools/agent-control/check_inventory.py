#!/usr/bin/env python3
"""Read-only source/inventory checks. Never connects to PartyLine or providers."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys

import catalog

INVENTORY = 'docs/partyline-agent-control.md'
NATIVE = 'frontend/src-tauri/src/agent_control/native.rs'
COMMAND = re.compile(r'#\[tauri::command(?:[^\]]*)\]\s*(?:#\[[^\]]+\]\s*)*pub\s+(?:async\s+)?fn\s+(\w+)\s*\(')
ROUTE = re.compile(r'\(\s*"partyline_(\w+)"\s*,\s*((?:"\w+"\s*(?:\|\s*)?)+),?\s*\)\s*=>')
ACTION = re.compile(r'`([a-z_]+/[a-z_]+)(?:\.[a-z_]+)?`')


def without_comments(source):
    source = re.sub(r'/\*.*?\*/', '', source, flags=re.S)
    return re.sub(r'(?m)^\s*//.*$', '', source)


def check(root, generated):
    errors = []
    stored = json.loads((root / 'frontend/src-tauri/src/agent_control/tools.json').read_text())
    if stored != generated:
        errors.append('Tool schemas or permissions drifted: run python3 tools/agent-control/catalog.py and review the diff.')
    expected = {f"{tool['name'].removeprefix('partyline_')}/{schema['properties']['action']['const']}"
                for tool in generated for schema in tool['inputSchema']['oneOf']}
    doc = (root / INVENTORY).read_text()
    section = doc.split('## Parity inventory\n', 1)[-1].split('\n## ', 1)[0]
    # Only the mapping table counts as coverage, not examples or maintenance prose.
    table = '\n'.join(line for line in section.splitlines() if line.startswith('|'))
    documented = set(ACTION.findall(table))
    for action in sorted(expected - documented):
        errors.append(f'Undocumented agent action: {action}')
    for action in sorted(documented - expected):
        errors.append(f'Stale inventory action: {action}')
    adapter = without_comments((root / NATIVE).read_text())
    implemented = {f'{tool}/{action}' for tool, actions in ROUTE.findall(adapter)
                   for action in re.findall(r'"(\w+)"', actions)}
    core = without_comments((root / 'frontend/src-tauri/src/agent_control/core.rs').read_text())
    if re.search(r'request\.tool\s*==\s*"partyline_status"\s*&&\s*action\s*==\s*"activity"', core):
        implemented.add('status/activity')
    for action in sorted(expected - implemented):
        errors.append(f'Agent action has no native/service route: {action}')
    for action in sorted(implemented - expected):
        errors.append(f'Native/service route has no tool schema: {action}')
    commands = set()
    for path in (root / 'frontend/src-tauri/src/cloud').rglob('*.rs'):
        commands.update(COMMAND.findall(without_comments(path.read_text())))
    documented_commands = set(re.findall(r'`(cloud_\w+)`', table))
    for command in sorted(commands):
        if command not in documented_commands:
            errors.append(f'Cloud command missing from inventory: {command}')
        if not re.search(r'\b' + re.escape(command) + r'\s*\(', adapter):
            errors.append(f'Cloud command missing from native adapter: {command}')
    for command in sorted(documented_commands - commands):
        errors.append(f'Stale inventory command: {command}')
    return errors


def runtime_change(path):
    p = Path(path)
    return ((path.startswith('frontend/src/') and p.suffix in {'.ts', '.tsx', '.js', '.jsx'})
            or (path.startswith('frontend/src-tauri/src/') and p.suffix == '.rs')
            or path.startswith(('frontend/src-tauri/permissions/', 'frontend/src-tauri/capabilities/'))
            or path in {'frontend/src-tauri/build.rs', 'frontend/src-tauri/tauri.conf.json',
                        'tools/agent-control/catalog.py', 'tools/agent-control/server.py'})


def review_errors(paths):
    if any(runtime_change(p) for p in paths) and INVENTORY not in paths:
        return ['UI/native behavior changed without an inventory review. Update docs/partyline-agent-control.md in the same change; describe unchanged mappings or remaining gaps when appropriate.']
    return []


def changed_paths(root, base):
    # Compare the branch's changes, including a local staged/unstaged working tree.
    ancestor = subprocess.check_output(['git', 'merge-base', base, 'HEAD'], cwd=root, text=True).strip()
    tracked = subprocess.check_output(['git', 'diff', '--name-only', '-z', ancestor], cwd=root)
    untracked = subprocess.check_output(['git', 'ls-files', '--others', '--exclude-standard', '-z'], cwd=root)
    return set((tracked + untracked).decode().strip('\0').split('\0')) - {''}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base', help='Base Git ref for the inventory-review check; includes local changes.')
    args = parser.parse_args()
    try:
        errors = check(catalog.ROOT, catalog.TOOLS)
        if args.base:
            errors += review_errors(changed_paths(catalog.ROOT, args.base))
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print(f'Could not check the parity inventory: {type(exc).__name__}', file=sys.stderr)
        return 1
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 1
    count = sum(len(t['inputSchema']['oneOf']) for t in catalog.TOOLS)
    print(f'Agent inventory matches {len(catalog.TOOLS)} tools, {count} actions, and the cloud commands.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

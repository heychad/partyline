"""Single source for the eight public tools and their per-action permissions."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / 'frontend/src-tauri/src/agent_control/tools.json'


def string(maximum, minimum=0, **extra):
    return dict(type='string', minLength=minimum, maxLength=maximum, **extra)


ID = string(36, 32, format='uuid')
MIC = string(512, description='Input name from partyline_audio/list; empty means system default.')
PEOPLE = dict(type='array', maxItems=100, items=string(80, 1))
OFFSET = dict(type='integer', minimum=0, maximum=1000000, default=0)
LIMIT = dict(type='integer', minimum=1, maximum=500, default=100)
BOOL = dict(type='boolean')


def action(name, permission, label, properties=None, required=()):
    props = {'action': {'const': name}, **(properties or {})}
    return dict(name=name, permission=permission, label=label, schema=dict(
        type='object', properties=props, required=['action', *required], additionalProperties=False))


def tool(name, description, actions):
    return dict(
        name=f'partyline_{name}', description=description,
        inputSchema=dict(type='object', oneOf=[a['schema'] for a in actions]),
        annotations=dict(readOnlyHint=all(a['permission'] == 'read' for a in actions),
                         destructiveHint=any(a['permission'] == 'control' for a in actions),
                         idempotentHint=False, openWorldHint=name in ('recording', 'notes', 'connections')),
        _meta={'partyline/permissions': {a['name']: a['permission'] for a in actions},
               'partyline/labels': {a['name']: a['label'] for a in actions}})


TOOLS = [
    tool('status', 'Read app and recording status or recent agent activity. Agent access is a saved human-controlled setting; tools cannot change it. Show, hide, or quit the app. Quit uses the normal recording drain; provide expected_meeting_id if a meeting is active.', [
        action('get', 'read', 'Read app status'),
        action('activity', 'read', 'Read agent activity'),
        action('show', 'control', 'Show PartyLine'),
        action('hide', 'control', 'Hide PartyLine'),
        action('quit', 'control', 'Quit PartyLine', {'expected_meeting_id': ID}),
    ]),
    tool('audio', 'List inputs, inspect timestamped live audio levels and connection warnings, or select the next microphone in the setup form. Diagnostics do not start capture. Device changes during recording are unsupported.', [
        action('list', 'read', 'List microphones'),
        action('diagnostics', 'read', 'Read audio diagnostics'),
        action('select', 'control', 'Select next microphone', {'microphone': MIC}, ['microphone']),
    ]),
    tool('meetings', 'Read cloud history, meeting state, or paginated transcript turns. Open a saved meeting, prepare the next meeting, control transcript following, or dismiss a banner. Raw text editing, deletion, and live participant-list edits are unsupported.', [
        action('list', 'read', 'Read meeting history', {'offset': OFFSET, 'limit': LIMIT}),
        action('get', 'read', 'Read meeting', {'id': ID}, ['id']),
        action('transcript', 'read', 'Read transcript', {'id': ID, 'offset': OFFSET, 'limit': LIMIT, 'final_only': BOOL}, ['id']),
        action('open', 'control', 'Open meeting', {'id': ID}, ['id']),
        action('new', 'control', 'Open new meeting'),
        action('prepare', 'control', 'Prepare meeting', {'title': string(200), 'participants': PEOPLE, 'microphone': MIC}),
        action('follow', 'control', 'Set transcript following', {'enabled': BOOL}, ['enabled']),
        action('dismiss', 'control', 'Dismiss banner', {'target': {'enum': ['error', 'notice']}}, ['target']),
    ]),
    tool('recording', 'Start, pause, resume, or stop cloud capture using the same native commands as the app. Start uses supplied fields (empty title, no suggestions, and default microphone if omitted). It sends audio to AssemblyAI and transcript text to OpenAI. Pause keeps provider sessions open. Stop returns before final notes finish; poll status or meetings/get.', [
        action('start', 'control', 'Start recording', {'title': string(200), 'participants': PEOPLE, 'microphone': MIC}),
        *[action(name, 'control', label, {'expected_meeting_id': ID}, ['expected_meeting_id'])
          for name, label in [('pause', 'Pause recording'), ('resume', 'Resume recording'), ('stop', 'Stop recording')]],
    ]),
    tool('speakers', 'Read participant suggestions and detected speaker IDs, or confirm a name. An empty name clears confirmation. ME and unidentified speakers cannot be renamed. Suggestions never assign identities automatically; confirmation updates transcript labels and invalidates notes.', [
        action('list', 'read', 'Read speakers', {'id': ID}, ['id']),
        action('rename', 'control', 'Confirm speaker name', {'id': ID, 'speaker': string(160, 1), 'name': string(80)}, ['id', 'speaker', 'name']),
    ]),
    tool('notes', 'Read saved and streaming notes with their revisions, or request a refresh. Refresh uses OpenAI and returns when queued; poll notes/get for completion. Stop requests final notes through the recording lifecycle.', [
        action('get', 'read', 'Read notes', {'id': ID}, ['id']),
        action('refresh', 'control', 'Refresh notes', {'id': ID}, ['id']),
    ]),
    tool('export', 'Export Markdown to the existing meeting folder and return it, copy that export to the Mac clipboard, or reveal the folder in Finder. No caller-supplied paths are accepted.', [
        action('markdown', 'control', 'Export meeting', {'id': ID}, ['id']),
        action('copy', 'control', 'Copy meeting export', {'id': ID}, ['id']),
        action('reveal', 'control', 'Reveal meeting files', {'id': ID}, ['id']),
    ]),
    tool('connections', 'Read only key-presence flags, save or replace keys in Keychain, test provider access, remove a saved key, or open/close Connections. Key values are write-only and never appear in results or activity. Test performs authentication/model-access checks, not paid transcription or notes.', [
        action('status', 'read', 'Read connection status'),
        action('save', 'control', 'Save connection keys', {'assemblyai': string(512, 1), 'openai': string(512, 1), 'test': BOOL}),
        action('test', 'control', 'Test connections'),
        action('remove', 'control', 'Remove connection key', {'provider': {'enum': ['assemblyai', 'openai']}}, ['provider']),
        action('open', 'control', 'Open Connections'),
        action('close', 'control', 'Close Connections'),
    ]),
]

if __name__ == '__main__':
    CATALOG.write_text(json.dumps(TOOLS, ensure_ascii=False, indent=2) + '\n')

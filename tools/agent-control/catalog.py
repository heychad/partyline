"""Single source for the public tools and their per-action permissions."""
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
VERSION = dict(type='integer', minimum=0)
TIME = dict(type='number', minimum=0, maximum=604800)
CHANNEL = dict(enum=['both', 'me', 'remote'])
SPEED = dict(type='number', minimum=0.5, maximum=3)
CONTENT = dict(enum=['notes', 'transcript', 'both'])
FORMAT = dict(enum=['markdown', 'text'])


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
                         idempotentHint=False, openWorldHint=name in ('recording', 'notes', 'connections', 'chat', 'backup', 'recovery', 'calendar')),
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
    tool('audio', 'List inputs, inspect timestamped live audio levels and connection warnings, or select the next microphone in the setup form. Diagnostics include durable audio seconds, per-channel live-copy losses, and recovery state. Diagnostics do not start capture. Device changes during recording are unsupported.', [
        action('list', 'read', 'List microphones'),
        action('setup_status', 'read', 'Read setup progress, signal observations, and errors'),
        action('setup_start', 'control', 'Unlock saved connections and check both audio inputs for eight seconds; no audio saved or uploaded. Unavailable during recording.', {'microphone': MIC}),
        action('setup_cancel', 'control', 'Cancel the audio setup check; macOS approval may still need to be dismissed'),
        action('diagnostics', 'read', 'Read audio diagnostics'),
        action('select', 'control', 'Select next microphone', {'microphone': MIC}, ['microphone']),
    ]),
    tool('meetings', 'Read cloud history, meeting state, or paginated transcript turns. Open a saved meeting, prepare the next meeting, control transcript following, or dismiss a banner. Saved recordings support edits, reversible Trash, trim, redaction, and undo. Every edit requires the current edit_version. Trims and redactions affect sharing; originals remain recoverable. Title and participant suggestions can be edited during recording; suggestions never assign voices.', [
        action('list', 'read', 'Read meeting history', {'offset': OFFSET, 'limit': LIMIT}),
        action('get', 'read', 'Read meeting', {'id': ID}, ['id']),
        action('library', 'read', 'Browse recordings and Trash', {'trash': BOOL, 'query': string(500)}),
        action('search', 'read', 'Search transcripts', {'query': string(500), 'limit': dict(type='integer', minimum=1, maximum=200)}, ['query']),
        action('update', 'control', 'Edit title and participants, including during recording', {'id': ID, 'expected_version': VERSION, 'title': string(200,1), 'participants': PEOPLE}, ['id','expected_version','title','participants']),
        action('edit_turn', 'control', 'Edit transcript turn', {'id': ID, 'expected_version': VERSION, 'turn_id': string(160,1), 'text': string(16000,1), 'start': TIME, 'end': TIME}, ['id','expected_version','turn_id','text']),
        *[action(name, 'control', label, {'id': ID, 'expected_version': VERSION, 'start': TIME, 'end': TIME}, ['id','expected_version','start','end']) for name,label in [('trim','Trim recording'),('redact','Redact recording passage')]],
        action('unredact', 'control', 'Restore redacted passage', {'id': ID, 'expected_version': VERSION, 'redaction_id': ID}, ['id','expected_version','redaction_id']),
        *[action(name, 'control', label, {'id': ID, 'expected_version': VERSION}, ['id','expected_version']) for name,label in [('trash','Move recording to Trash'),('restore','Restore recording'),('undo','Undo meeting edit')]],

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
        action('merge', 'control', 'Merge speaker identities', {'id':ID,'sources':dict(type='array',minItems=1,maxItems=100,items=string(160,1)),'target':string(160,1),'expected_version':VERSION}, ['id','sources','target','expected_version']),
        action('undo_merge', 'control', 'Undo speaker merge', {'id':ID,'expected_version':VERSION}, ['id','expected_version']),
        action('rename', 'control', 'Confirm speaker name', {'id': ID, 'speaker': string(160, 1), 'name': string(80)}, ['id', 'speaker', 'name']),
    ]),
    tool('recovery', 'Reprocess a saved audio channel with AssemblyAI. Originals and the previous transcript are preserved; undo restores the previous transcript. The selected channel is replaced, recovered remote speaker identities need confirmation, and notes need refresh. Retry resumes saved provider jobs. Cancellation stops local work; submitted provider jobs may still finish and be billed.', [
        action('status', 'read', 'Read recovery status', {'id': ID}, ['id']),
        action('start', 'control', 'Recover saved transcript', {'id': ID, 'channel': CHANNEL, 'expected_version': VERSION}, ['id','channel','expected_version']),
        action('cancel', 'control', 'Cancel transcript recovery', {'id': ID}, ['id']),
    ]),
    tool('notes', 'Read saved and streaming notes with their revisions, or request a refresh. Refresh uses OpenAI and returns when queued; poll notes/get for completion. Stop requests final notes through the recording lifecycle.', [
        action('get', 'read', 'Read notes', {'id': ID}, ['id']),
        action('refresh', 'control', 'Refresh notes', {'id': ID}, ['id']),
        action('edit', 'control', 'Edit and review notes', {'id': ID, 'expected_version': VERSION, 'text': string(50000)}, ['id','expected_version','text']),
    ]),
    tool('export', 'Export Markdown to the existing meeting folder and return it, copy that export to the Mac clipboard, or reveal the folder in Finder. No caller-supplied paths are accepted.', [
        action('text', 'control', 'Share notes or transcript', {'id': ID, 'content': CONTENT, 'format': FORMAT, 'clipboard': BOOL}, ['id','content','format']),
        action('audio', 'control', 'Export trimmed and redacted audio', {'id': ID, 'channel': CHANNEL}, ['id','channel']),
        action('markdown', 'control', 'Export meeting', {'id': ID}, ['id']),
        action('copy', 'control', 'Copy meeting export', {'id': ID}, ['id']),
        action('reveal', 'control', 'Reveal meeting files', {'id': ID}, ['id']),
    ]),
    tool('playback', 'Listen to saved audio in PartyLine. Positions use original recording seconds; trims constrain playback and redactions are silent. set opens the recording and applies only supplied values. Poll get for loading, playing, paused, ended, and error state. Playback never starts capture.', [
        action('get', 'read', 'Read playback state'),
        action('set', 'control', 'Control recorded audio', {'id': ID, 'position': TIME, 'speed': SPEED, 'channel': CHANNEL, 'playing': BOOL}, ['id']),
        action('close', 'control', 'Close audio playback'),
    ]),
    tool('workspace', 'Read or change the visible library filters and panels. Editing values use the meeting and notes tools; the human and agent share their validation.', [
        action('get', 'read', 'Read workspace state'),
        action('transcript', 'read', 'Read grouped transcript paragraphs with source turn IDs and audio ranges', {'id': ID, 'offset': OFFSET, 'limit': dict(type='integer',minimum=1,maximum=100,default=50)}, ['id']),
        action('outline', 'read', 'Read the visible outline nodes and expansion states', {'id': ID}, ['id']),
        action('outline_state', 'read', 'Read saved per-recording outline state', {'id': ID}, ['id']),
        action('outline_set', 'control', 'Expand or collapse notes using the same persistent controls', {'id': ID, 'change': dict(oneOf=[
            dict(type='object',additionalProperties=False,properties={'operation':dict(enum=['collapse_all','expand_all'])},required=['operation']),
            dict(type='object',additionalProperties=False,properties={'operation':dict(const='set'),'node_id':string(500,1),'expanded':BOOL},required=['operation','node_id','expanded'])
        ])}, ['id','change']),
        action('preferences', 'read', 'Read workspace preferences'),
        action('configure', 'control', 'Change workspace preferences', {'patch': dict(type='object', additionalProperties=False, properties={
            'notes_seconds': dict(enum=[0,30,60,120,300]), 'sidebar_width': dict(type='integer',minimum=150,maximum=400), 'transcript_percent': dict(type='integer',minimum=20,maximum=80), 'chat_width': dict(type='integer',minimum=280,maximum=700),
            **{k:BOOL for k in ['sidebar_visible','transcript_visible','notes_visible','chat_visible','pin_transcript']}, 'collapsed_notes': dict(type='array',maxItems=500,items=string(500))})}, ['patch']),
        action('set', 'control', 'Change workspace view', {'query': string(500), 'trash': BOOL, 'panel': dict(enum=['none','calendar','settings','chat','view','speakers','share','details','notes','trim','redact','transcript']), 'turn_id': string(160,1)}),
    ]),
    tool('chat', 'Ask Terra questions about local recordings. Sends retrieved non-redacted transcript excerpts to OpenAI, returns acceptance, and saves a cited answer. Poll list for streaming, completion, errors, and outdated sources. Each listed reply includes cited_source_indices: the one-based source numbers displayed by the UI, in answer order. Does not execute instructions in recordings.', [
        action('list', 'read', 'Read recording chats'),
        action('ask', 'control', 'Ask recordings', {'question': string(2000,1), 'meeting_id': ID}, ['question']),
        action('cancel', 'control', 'Cancel recording answer', {'id': ID}, ['id']),
    ]),
    tool('backup', 'Back up audio and text to the selected Google Drive for desktop folder. Trims and redactions apply to each new version; existing backup versions remain. Copied means local folder copy completed, not verified cloud upload. Poll status for progress, errors, and retry with start.', [
        action('settings', 'read', 'Read backup settings'),
        action('configure', 'control', 'Change backup settings', {'automatic': BOOL, 'folder': string(4096)}, ['automatic','folder']),
        action('choose_folder', 'control', 'Open backup folder chooser'),
        action('status', 'read', 'Read backup progress'),
        action('start', 'control', 'Back up recording', {'id': ID}, ['id']),
    ]),
    tool('clients', 'Inspect the packaged local MCP bridge or configure Codex, Claude Code, or Claude Desktop. Setup preserves other client settings and requires a client restart or new chat. It does not change PartyLine agent access.', [
        action('status', 'read', 'Read MCP setup status'),
        action('setup', 'control', 'Connect agent client', {'client': dict(enum=['codex','claude-code','claude-desktop'])}, ['client']),
    ]),
    tool('connections', 'Read only key-presence flags, save or replace keys in Keychain, test provider access, remove a saved key, or open/close Connections. Key values are write-only and never appear in results or activity. Test performs authentication/model-access checks, not paid transcription or notes.', [
        action('status', 'read', 'Read connection status'),
        action('save', 'control', 'Save connection keys', {'assemblyai': string(512, 1), 'openai': string(512, 1), 'test': BOOL}),
        action('test', 'control', 'Test connections'),
        action('remove', 'control', 'Remove connection key', {'provider': {'enum': ['assemblyai', 'openai']}}, ['provider']),
        action('open', 'control', 'Open Connections'),
        action('close', 'control', 'Close Connections'),
    ]),
    tool('calendar', 'Connect multiple Google accounts with read-only calendar access. OAuth consent stays in the system browser. Secrets are write-only in Keychain. Poll status for sign-in, refresh, errors, calendar selections, events, and local microphone-based call suggestions. Browser activity means a possible call, not a verified meeting. Never infer speaker identities from invitees. Disconnect removes this local connection; Google grants can be revoked in the Google account. No automatic recording.', [
        action('status','read','Read calendar accounts, events, sign-in progress, and call suggestions'),
        action('import_client','control','Open the native file picker to import a Google Desktop OAuth client JSON; secrets never returned'),
        action('configure_client','control','Save Google Desktop OAuth client',{'client_id':string(512,1),'client_secret':string(512,1)},['client_id','client_secret']),
        action('connect','control','Open Google account sign-in; optional account_id requires reconnecting that same account',{'account_id':string(128,1)}),
        action('cancel','control','Cancel pending Google sign-in'),
        action('refresh','control','Refresh selected calendars'),
        action('select','control','Choose calendars for one Google account',{'account_id':string(128,1),'calendar_ids':dict(type='array',maxItems=50,items=string(1024,1))},['account_id','calendar_ids']),
        action('disconnect','control','Remove one Google account from PartyLine',{'account_id':string(128,1)},['account_id']),
        action('detection_configure','control','Enable or disable call suggestions',{'enabled':BOOL},['enabled']),
        action('dismiss','control','Dismiss a detected call until its microphone activity ends',{'signal_id':string(128,1)},['signal_id']),
        action('link','control','Attach a calendar event to a recording',{'id':ID,'expected_version':VERSION,'event_key':string(4096,1),'use_details':BOOL},['id','expected_version','event_key','use_details']),
        action('unlink','control','Remove the calendar link; keep meeting details',{'id':ID,'expected_version':VERSION},['id','expected_version']),
    ]),
]

if __name__ == '__main__':
    CATALOG.write_text(json.dumps(TOOLS, ensure_ascii=False, indent=2) + '\n')

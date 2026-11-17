# PartyLine agent control

Fifteen MCP tools cover 87 actions in the cloud workspace and its native commands. They call the same recording, speaker, notes, export, and Keychain functions as the app. A small view bridge handles setup fields, history selection, Connections, banners, and transcript following.

The implementation is in `frontend/src-tauri/src/agent_control/`, `frontend/src/components/agent-control/`, and `tools/agent-control/`. The macOS app and DMG rebuilt on November 14, 2026 include persistent access settings. The previous 33-action catalog has live acceptance coverage across the normal app and an isolated native key-test profile; the expanded library catalog has automated and local live coverage, including a completed AI-answer check on November 15. New installations start with agent access Off.

## Parity inventory

Inventory source: `CloudWorkspace.tsx`, `cloud/mod.rs`, `cloud/keys.rs`, `lib.rs`, and `tray.rs` on the working cloud branch.

| Human action or visible state | Agent action | Existing implementation |
| --- | --- | --- |
| App status, active meeting, connection state | `status/get` | `cloud_current`, `cloud_key_status` |
| Open or hide the window; quit normally | `status/show`, `status/hide`, `status/quit` | Main window methods; existing exit handler drains capture and notes |
| List microphones | `audio/list` | `get_audio_devices` |
| Select the next microphone | `audio/select` or `recording/start.microphone` | Setup form; `cloud_start` |
| Live microphone/Mac meters, elapsed time, warnings | `audio/diagnostics` | Existing `cloud-levels` events and `cloud_current` |
| One-click recording; optional prepared title and participant suggestions | `meetings/new`, `meetings/prepare` or `recording/start` | Quick Start and sidebar start; `cloud_start` |
| Start, pause, resume, stop | `recording/start`, `recording/pause`, `recording/resume`, `recording/stop` | `cloud_start`, `cloud_pause`, `cloud_stop` |
| History and saved meeting selection | `meetings/list`, `meetings/get`, `meetings/open` | `cloud_list`, `cloud_get`; existing UI open handler |
| Live/final transcript, word timings, revised speaker IDs | `meetings/transcript` | Current meeting snapshot and ordered turns |
| Follow transcript; dismiss error or notice | `meetings/follow`, `meetings/dismiss` | Existing React state setters |
| Participant suggestions and confirmed speaker names | `speakers/list`, `speakers/rename` | Meeting participants/names; `cloud_rename_speaker` |
| Running draft, saved notes, status, errors, revisions | `notes/get` | `cloud_get` |
| Refresh notes; final notes on stop | `notes/refresh`; `recording/stop` | `cloud_refresh_notes`; normal recording drain |
| Export notes and transcript to Markdown | `export/markdown` | `cloud_export` |
| Copy the export | `export/copy` | Same export plus macOS clipboard; notice in the window |
| Reveal saved files | `export/reveal` | `cloud_reveal` |
| Open/close Connections; key-presence flags | `connections/open`, `connections/close`, `connections/status` | Existing dialog; `cloud_key_status` |
| Save keys; save and test; test separately | `connections/save` with optional `test`; `connections/test` | `cloud_save_keys`, `cloud_test_keys` |
| Remove a saved key (native command, no current button) | `connections/remove` | `cloud_remove_key` |
| See agent actions and outcomes | `status/activity` | Bounded activity log in the **Agents** control |
| Search library and view Trash | `meetings/library`, `meetings/search`; `workspace/get`, `workspace/set` for visible filters | `cloud_library`, `cloud_search`; shared React filters |
| Edit title and participants during recording; edit saved transcript and notes | `meetings/update`, `meetings/edit_turn`, `notes/edit` | `cloud_edit`; shared version-checked edit and atomic persistence |
| Trim, redact, restore a redacted passage, undo | `meetings/trim`, `meetings/redact`, `meetings/unredact`, `meetings/undo` | `cloud_edit`; original files and revision history retained |
| Move to Trash and restore | `meetings/trash`, `meetings/restore` | `cloud_edit`; recoverable deletion, no purge action |
| Click transcript to play; pause, seek, source, speed, close | `playback/get`, `playback/set`, `playback/close` | Shared playback controller and `cloud_audio`; state includes loading/error/end |
| Copy/export notes, transcript, both; export edited audio | `export/text`, `export/audio` | `cloud_share`, `cloud_audio`; trims/redactions and notes-review guard |
| Open/close edit, share, Settings, chat panels | `workspace/get`, `workspace/set` | Shared window callbacks; direct edits use the same native commands |
| Ask recordings, see streamed answer/history, cancel, retry | `chat/ask`, `chat/list`, `chat/cancel` | `cloud_chat_ask`, `cloud_chat_list`, `cloud_chat_cancel`; retry asks again |
| Choose Drive folder and automatic backup | `backup/settings`, `backup/configure`, `backup/choose_folder` | `cloud_backup_settings`, `cloud_backup_configure`, `cloud_backup_choose_folder`; picker remains visible |
| Start/retry backup, see progress/errors | `backup/start`, `backup/status` | `cloud_backup_start`, `cloud_backup_status`; async local copy, Drive uploads separately |
| Inspect MCP setup and connect clients | `clients/status`, `clients/setup` | `cloud_client_status`, `cloud_client_setup`; preserves other client configuration |
| Merge live or saved speaker identities; undo merge | `speakers/merge`, `speakers/undo_merge`; `speakers/list` returns aliases and speaker edit version | `cloud_merge_speakers`, `cloud_undo_speaker_merge`; shared validation and persisted aliases; original provider IDs retained |
| Recover saved transcript; cancel/retry and read progress | `recovery/start`, `recovery/cancel`, `recovery/status` | `cloud_recovery_start`, `cloud_recovery_cancel`, `cloud_get`; control required for provider requests, read for progress |
| Resize/show/hide panels, pin transcript, choose notes interval | `workspace/preferences`, `workspace/configure` | `cloud_preferences`, `cloud_preferences_update`; same validated persisted settings and events for UI/tools |
| Check setup before a meeting; cancel or retry | `audio/setup_status`, `audio/setup_start`, `audio/setup_cancel` | `cloud_setup_status`, `cloud_setup_start`, `cloud_setup_cancel`; shared capture exclusion, progress/events, Keychain unlock, discarded audio samples |
| Read the grouped transcript with paragraph boundaries, original turn IDs, and audio ranges | `workspace/transcript`; raw turns remain at `meetings/transcript` | Same `transcriptGroups`/`transcriptPage` functions rendered by `TranscriptPane` |
| Read/collapse/expand the notes outline, including all nested nodes and future additions | `workspace/outline`, `workspace/outline_state`, `workspace/outline_set` | `cloud_outline`, `cloud_outline_update`; shared node validation, atomic per-meeting state, `cloud-outline` event |
| Open View and speaker-management popovers | `workspace/get`, `workspace/set` with panel `view` or `speakers`; `speakers/rename`, `speakers/merge` perform edits | Shared controlled popovers and native speaker commands |
| Google account status, selected calendars, upcoming events, sync/sign-in errors | `calendar/status` | `cloud_calendar_action`; tokens never returned; account-specific errors and selections |
| Configure Desktop OAuth client; add/reconnect Google accounts; cancel pending sign-in | `calendar/import_client`, `calendar/configure_client`, `calendar/connect`, `calendar/cancel` | `cloud_calendar_action`; Keychain, system-browser PKCE/loopback OAuth, human Google consent |
| Choose calendars, refresh, remove one account | `calendar/select`, `calendar/refresh`, `calendar/disconnect` | `cloud_calendar_action`; same persisted per-account selection, cache removal, events, and validation |
| Enable/disable microphone-based call suggestions; dismiss a suggestion | `calendar/detection_configure`, `calendar/dismiss`; `calendar/status` reads evidence/errors | `cloud_calendar_action`; debounced Core Audio process activity, no audio captured by detector |
| Attach/unlink an event during or after recording; optionally use title/invitees | `calendar/link`, `calendar/unlink` | `cloud_calendar_link`; shared version-checked calendar edit, no voice inference |
| Open/close calendar panel; record from a detected-call prompt | `workspace/set` with panel `calendar` or `none`; `recording/start` | Shared window callbacks; ordinary recording command |
| Grant/revoke agent access; saved level across restarts | Human-only **Agents** control; `status/get` reports the level | Main-window permission commands and private access settings; agents cannot grant access |

Tool names have the `partyline_` prefix. For example, `notes/get` means `partyline_notes` with `{"action":"get","id":"…"}`. The machine-readable catalog is `frontend/src-tauri/src/agent_control/tools.json`; `catalog.py` generates it.

Unsupported actions are explicit in `status/get`: changes to the live microphone and update checks. Live title and participant suggestions are editable; live audio-input changes remain unsupported. Permanent deletion is intentionally absent; Trash is recoverable. The tray dispatches an update event, but the cloud root does not mount its listener. Legacy local-model screens and commands are outside this cloud API.

Participant suggestions never identify a speaker automatically. Rename a detected speaker only after confirmation; an empty name removes confirmation. Existing provider revisions and reconnect speaker IDs remain authoritative.

## Access and secrets

The app listens on an owner-only Unix socket:

```text
~/Library/Application Support/com.heychad.partyline/agent-control/control.sock
```

The directory is `0700`, the socket is `0600`, and the listener checks peer ownership. There is no TCP listener, browser endpoint, or network binding. The stdio bridge verifies socket ownership and permissions before connecting. It refuses symlinks and unsafe permissions.

New installations start with agent access **Off**. In the sidebar, **Agents** lets the human choose **Off**, **Read only**, or **Control**. The choice applies to processes under that macOS account and survives app restarts until the human changes it. Only the main local app window can change it; no agent tool grants itself access. Disabling access rejects queued operations that have not started. An operation already underway can finish.

The setting is stored atomically in `agent-control/access.json`, with file permissions `0600` inside the private `0700` agent directory. Missing settings mean Off. Invalid or unsafe settings fail closed to Off and show a recovery message. A failed save leaves the current grant unchanged and reports an error. The file contains only a format version and access level, never keys or meeting content.

Read access includes meeting content and key-presence flags. Control also permits recording, provider requests, exports, clipboard changes, and key changes. The interface states this scope. Every accepted or denied, well-formed action appears in the activity log. The log holds the latest 100 actions in memory and contains fixed action labels, timestamps, and outcomes—no arguments, transcript text, participant names, or keys.

Keys are write-only inputs to `connections/save`. Connection results allow only booleans and success status. Connection errors never return backend response bodies or input values. There is no key-reading tool, arbitrary native-command tool, caller-selected export path, or direct meeting-file writer.

## Recording and diagnostics

`recording/start` accepts `title`, `participants`, and `microphone`. Omitted values mean a generated title, no suggestions, and the system-default microphone. Setup preparation changes what the human sees before their next start; agent starts use the supplied arguments.

Pause, resume, and stop require `expected_meeting_id`, checked against the active meeting before invoking the shared command. Agent mutations are serialized. Human commands retain their existing native behavior. The API does not add an atomic session-ID guard to the underlying capture implementation.

Start, stop, and notes refresh return acceptance, not completion. Poll `status/get`, `meetings/get`, or `notes/get` to observe connection, finishing, completion, and notes errors. A stopped meeting can still be producing its final notes. Pausing keeps the provider sessions open.

Diagnostics return the latest existing level event, its observation time, its age, connection states, and warnings. A sample older than two seconds is marked stale. No sample is distinct from silence. No diagnostic request opens a device or triggers permission prompts. The current capture API does not report the actual active microphone name, so the tool explicitly reports it as unknown.

Transcript pages contain raw turns and word-level speaker IDs plus confirmed names and the meeting revision. Re-read affected pages when the revision changes; page offsets are not stable cursors during live transcription or provider corrections. `meetings/get` returns the full snapshot for smaller meetings.

The bridge never retries a failed mutation automatically. A lost response can mean the action ran; inspect app state before retrying. Requests are limited to 512 KiB, responses to 16 MiB, and simultaneous socket exchanges to eight. Use transcript pagination for long meetings.

## Wiring and use

`lib.rs` registers the plugin. `CloudWorkspace.tsx` mounts the access control and view bridge. `build.rs` and `tauri.conf.json` register and allow four plugin commands for the main local window: permission status, permission changes, view state, and view acknowledgements. The renderer has no generic native-command proxy.

Build and launch the app only after the live meeting has ended and its notes are saved. Do not overwrite the running app to test this feature. Choose access once in **Agents**; it stays selected across restarts. Use **Control** to test a disposable meeting's start, pause, resume, rename, stop, notes refresh, and export. Check that the human UI updates throughout. Permission-level and revocation checks belong in an isolated profile or a separately authorized acceptance run; ordinary use does not require toggling access.

Configure an MCP client to launch this stdio command:

```sh
python3 /Users/heychad/code/partyline/tools/agent-control/server.py
```

No API keys or socket token belong in the MCP configuration. Discovery works while the app is closed; tool calls report that app access is unavailable. A `--socket` override supports isolated test instances.

Example tool calls:

```json
{"name":"partyline_status","arguments":{"action":"get"}}
{"name":"partyline_audio","arguments":{"action":"diagnostics"}}
{"name":"partyline_recording","arguments":{"action":"start","title":"Rehearsal","participants":["Chad","Nicole"],"microphone":""}}
{"name":"partyline_recording","arguments":{"action":"stop","expected_meeting_id":"REPLACE-WITH-RETURNED-UUID"}}
```

## Maintenance and CI

[AGENTS.md](../AGENTS.md) makes agent parity part of the definition of done. Update this table, tool schemas, adapter, permission rules, and behavior tests together. Keep every tool/action explicit in the table so the drift check can compare it with the catalog and adapter. Document unsupported behavior and unresolved parity gaps; a passing inventory check does not make an unsupported feature complete.

`.github/workflows/agent-parity.yml` runs on pull requests and pushes that change UI, native code, dependencies, agent tools, or these instructions. It checks the generated schemas/permissions, every documented action, native/service routes, and public cloud commands. It also requires an inventory update when a change touches UI/native implementation or the tool catalog/bridge. For a refactor with unchanged mappings, record that review here.

The workflow runs the isolated Rust permission/socket tests, Python MCP tests, UI callback tests, and frontend type check. It never launches PartyLine, connects to its live socket, reads recordings or Keychain, or calls a provider. Dependency installation is the only network access these checks need.

These are complementary checks: source matching catches inventory drift; behavior tests check inputs, outcomes, permissions, errors, revocation, and UI callbacks. The isolated Rust tests use a fake native backend, so they do not establish end-to-end equivalence of the Tauri adapter and capture/notes lifecycle. The live checks below cover the tested operations; remaining gaps and unsupported behavior still need explicit review. A reviewer must verify that changed behavior has adequate tests; matching names alone is insufficient.

The maintenance review covers all current tool actions and public cloud commands. The November 14 update adds persistent human-controlled grants, matching UI/tool descriptions, and restart, revocation, invalid-file, and failed-save tests. Tool action names and required permission levels are unchanged.

## Verification

Run without launching PartyLine, reading its saved meetings, accessing Keychain, or calling a provider:

```sh
python3 tools/agent-control/check_inventory.py --base HEAD
cargo test --manifest-path tools/agent-control/Cargo.toml --locked --offline -j 1
python3 -m unittest discover -s tools/agent-control/tests -p 'test_*.py' -v
node --test tools/agent-control/tests/*.test.mjs
cargo test --manifest-path tools/meeting-library/Cargo.toml --locked --offline -j 1
cd frontend
./node_modules/.bin/tsc --noEmit --incremental false
```

The Rust tests exercise the actual permission service and socket with fake native operations. Python tests cover MCP initialization, discovery, framing, errors, private-socket transport, and a real stdio subprocess. UI tests exercise the view callbacks and active-meeting guard. A source inventory check covers every current `cloud_*` command. This CI job does not build the full desktop application; live checks use a separate disposable meeting after any real meeting has ended.

## Live verification — November 13, 2026

The app and DMG were built from `e7c1fc0`. Code-signature and disk-image checks passed, and the executable inside the DMG matched the app that was tested. A real MCP stdio client exercised 31 of the 33 actions against that app.

- A 52.3-second rehearsal verified start, pause, resume, stop, fresh audio diagnostics, Mac-audio transcription, a confirmed speaker name, running notes, final notes, and a separate notes refresh. The human UI showed the transcript, name, notes, and agent activity.
- Setup, microphone selection, Connections, follow, banner dismissal, window show/hide, history, transcript pagination, reopening, Markdown export, clipboard copy, and revealing files succeeded. The exported Markdown matched the saved file. Both audio files had matching durations.
- Off denied reads and mutations. Read only allowed reads but denied recording, speaker changes, notes refresh, and export. Attempts to replace an active meeting, switch its microphone, or pause a different meeting were rejected. Idle quit closed the socket; reopening reset access to Off and preserved the rehearsal and its notes. The November 14 update replaces that reset behavior with the saved human choice.

This first run left `connections/save`, `connections/remove`, and microphone speech unverified. The November 14 checks below cover them without changing the normal app's saved keys.

## Live verification — November 14, 2026

**33 of 33 actions verified.** The remaining key actions ran through the real MCP bridge, native adapter, and macOS Keychain in a disposable `com.heychad.partyline.key-tests` app. Its runtime source matched the normal app except for the Keychain service namespace; its bundle identity and app data were also separate. No provider requests used the generated test keys.

- Saving each provider independently, replacing one key without changing the other, and rejecting invalid input passed. Both saved keys survived a full app restart.
- Removing either provider preserved the other. Repeated removal returned an error without changing the remaining key. Both test keys were removed at the end, and the Connections UI updated to show both missing. Tool results and activity exposed no key values.
- The normal app's existing AssemblyAI and OpenAI connections still passed their connection tests after the rebuild. Their keys were neither replaced nor removed.
- **Control survived a full restart in both apps.** The normal app remains on Control, with the saved access file at `0600` in a `0700` directory. Its UI and MCP status agree. No agent action can change the grant.
- A separate 22.7-second microphone check used **System default**, identified by the user as the MacBook Pro microphone. Four completed turns appeared only as **ME**, and notes generated and survived restart. The microphone WAV contained speech (peak 0.891); the Mac-audio WAV contained silence (peak 0). Both were 48 kHz mono with matching durations. This verifies no duplicate capture in this test; the capture API still does not report the actual input-device name.

The silent Mac channel triggered the existing no-audio warning during the mic-only check. That warning does not establish a permission failure. Mac-audio transcription had passed the November 13 playback test. Full multi-participant diarization, other microphone routing configurations, native failure recovery, and simultaneous human/agent mutation races remain unverified.

The updated source passed 12 Rust tests, 16 Python tests, four UI tests, the frontend type check, the inventory check, and the production app/DMG build. The release signature and disk image verified. Local reports are `target/agent-control-key-{seed,restart,remove}.json`, `target/agent-control-mic-acceptance.json`, and `target/agent-control-acceptance.json`.

To repeat the native key test, build a disposable app with bundle ID `com.heychad.partyline.key-tests` and Keychain service `com.heychad.partyline.key-tests.cloud`, then choose Control there. Run `python3 tools/agent-control/tests/live_keys.py seed`, fully quit and reopen that app, run the helper with `verify`, then with `remove`. The helper only connects to the isolated profile, generates non-authenticating inputs in memory, and never prints or logs them. It is a manual acceptance check, not part of CI.

The cached UI view can briefly lag a successful action acknowledgement; poll status for the resulting view state. Local evidence is in `target/agent-control-acceptance.json`.

Protocol references: [MCP stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) and [MCP tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).

## Recording library expansion — November 14, 2026

The expanded catalog has 13 tools and 61 actions. The earlier 33-action acceptance applies to the previous build. The library additions have the coverage below; do not treat the catalog count as 61 completed live checks.

Edits use `edit_version` to reject stale requests and the same native command for both clients. Editing, Trash, trimming, and redaction require a finished recording. Undo restores a saved revision. Originals remain intact. Shared transcript/audio apply exclusions; earlier notes require refresh or explicit review before sharing. Raw local meeting reads still include original passages because redaction is a sharing control, not permanent erasure. Existing copies and prior Drive backup versions remain unchanged.

Playback positions use original recording seconds even after trimming. Playback uses edited WAVs, silences redacted ranges, and never starts capture. Human and agent playback use the same controller. Source, speed, position, loading, completion, and errors are visible through `playback/get` and the main UI.

Chat sends retrieved, non-redacted transcript excerpts to the existing OpenAI connection. It records the searched-recording and selected-passage counts, includes clickable source records, and excludes Trash. Answers based on changed or deleted recordings are marked outdated and their content is withheld until asked again. Chat returns an accepted job before reading Keychain, exposes `phase` (`unlocking` or `answering`) while running, and supports cancellation during either phase. macOS approval remains human-controlled. Chat does not execute instructions from recordings. Search retrieval is bounded; it does not establish exhaustive review of the whole library.

Drive backup stays inactive until a folder is selected in Settings. Automatic backup is on by default, as requested. Destinations must be within Google Drive for desktop. To disconnect through the tool, pass an empty `folder` string to `backup/configure`; `backup/settings` returns `folder: null`. Backup copies current audio/text as separate versions and reports local copy completion; PartyLine does not claim that Drive's cloud upload has finished. Failures and interrupted copies remain visible and retryable. Local originals and prior backups are preserved.

The MCP bridge is bundled with the app. Client setup installs a stable copy into app data and configures Codex, Claude Code, or Claude Desktop. Only PartyLine's entry changes; the Desktop configuration receives a private backup. The client must reload its MCP configuration. Setup does not grant PartyLine access or expose provider keys.


### Library acceptance

- Isolated checks: 12 agent permission/socket tests, 12 meeting/model tests, 19 Python tests, 11 UI/playback tests, TypeScript, and the 13-tool/61-action inventory check. The final production app and DMG build successfully. The app signature and DMG checksum verify, and the executable and packaged MCP resources inside the DMG match the tested app.
- Real MCP calls against a disposable six-second recording passed details/transcript/notes edits, stale-version rejection, trims, redaction/unredaction, undo, Trash/restore, search, export, and clipboard checks. Exported audio had the expected duration and exact silence over the redacted range; source WAVs were unchanged.
- Playback passed play, pause, seek, speed and channel changes, invalid-seek rejection, and close through the actual native webview. Human title and notes edits were verified through MCP; agent edits appeared in the UI; human Copy matched the native export exactly.
- The native folder picker opened from both UI and MCP; cancellation kept the destination unchanged. A disposable Google Drive folder received both trimmed/redacted audio tracks, transcript, notes, and metadata. Automatic backup on an edit and repeat backup succeeded. This verifies local Drive-folder copying, not completion of Google's cloud upload. The final setting is automatic backup enabled with no folder selected.
- MCP setup registered Codex, Claude Code, and Claude Desktop. The installed bridge discovered all 13 tools and called the live app. Claude Code's CLI reported Connected. Claude configurations retained their other MCP entries; repeating Codex setup preserved all other entries. The initial Codex preservation hash changed during the test interval, so that first-install comparison is inconclusive. Existing client chats may need a restart or new chat to load the configuration.
- A macOS Keychain prompt stopped the first live AI-answer check before any answer was accepted. The implementation now accepts the job before unlocking so progress and cancellation remain available. Live job acceptance and cancellation while Keychain is pending now pass, and the cancelled status appears in the UI. Clicking a retrieved source opens and plays the correct audio; changing its recording marks the stored reply outdated and clears the returned answer/sources. A regression test covers opening a different recording during playback load. On November 15, after the human approved Keychain, the existing request completed with the correct decision and owner and valid citations. Its answer appeared in the app. The helper resumed that same reply, verified its result, invalidated it by changing the source, and verified cancellation of a subsequent answer. No duplicate request was used to finish the pending test. Cross-recording retrieval also has a shared-library regression test covering multiple calls, Trash, and redactions.

Local evidence is in `target/partyline-library-live.json` and `target/partyline-library-preservation.json`. The repeatable manual helper is `tools/meeting-library/tests/live_library.py`; it requires the explicitly disposable fixture pointer in `target/partyline-library-fixture.json`. It never reads or prints API-key values. Real recordings were not used for these tests. All nine pre-existing meeting/audio files matched their pre-test hashes after validation. The verified app was installed at `/Applications/PartyLine.app`; its recordings, saved connections, and Control grant remained available.

References: [Tauri asset URLs](https://v2.tauri.app/reference/javascript/api/namespacecore/#convertfilesrc), [Codex MCP setup](https://developers.openai.com/codex/mcp), and [Claude Desktop local MCP setup](https://modelcontextprotocol.io/docs/develop/connect-local-servers).

## Recording reliability and workshop controls — November 17, 2026

Version 0.4.2 removes the three-second local queue stop. The recorder owns a separate runtime and writes samples on arrival. It never waits on transcript, notes, Keychain, or UI publication. Streaming copies have a bounded queue; loss there sets `recovery_needed`, while the local WAV continues. Headers and disk data are checkpointed every two seconds. Diagnostics and meeting reads expose `capture`: durable seconds, per-channel seconds, live-copy loss, levels, arrival flags, and update time. Fatal device or disk failures remain possible and produce a visible interrupted state and a desktop notification when macOS permits it.

Streaming reconnects with backoff up to 30 seconds and renews sessions before the provider's three-hour limit. A reconnect creates a new speaker namespace. Recording can start while provider connections are unavailable. Recovery is explicit and uses the selected original audio, including material excluded from sharing. It replaces that channel's transcript, retains a revision for undo, preserves audio originals, and requires notes review. Remote speaker names from a new transcription require confirmation. Progress, failure, cancellation, and retry are available through both UI and agent tools. Retry reuses saved provider IDs; if submission acknowledgement is lost before an ID is saved, inspect provider jobs before retrying.

The notes interval is 30, 60, 120 (default), or 300 seconds; 0 means manual. Explicit refresh and final notes remain available. Outline collapse IDs and panel widths/visibility persist. Transcript pinning is distinct from Follow live. Chat is a docked panel. New live turns do not invalidate an answer about unchanged cited passages; changed, redacted, trimmed, or deleted cited content does.

Validation evidence and remaining limits are recorded in [the recording reliability report](partyline-recording-reliability.md). A three-hour accelerated PCM test is not a three-hour hardware soak. Current microphone switching and device reconnection remain outside the supported live controls.

### Stable macOS signing

Local builds use the installed Apple Development identity instead of an ad-hoc signature. macOS still owns Keychain and audio grants. Approve the signed app with Always Allow during connection setup; repeated-launch checks must verify persistence. No agent permission or key-value API changes. Other developers must supply their own signing identity.

## Live workspace — 0.4.3

The compact View menu controls panel visibility and independent scrolling. Library, transcript/notes, and Ask dividers preview their size while dragging and save on release; arrow keys also resize them. Speaker management is a popover. Consecutive turns from the same confirmed speaker share a sticky heading. Presentation-only sentence grouping retains source turn IDs and audio ranges for playback, edits, and redaction. Follow live observes both content growth and pane resizing; upward scrolling pauses it until resumed.

Outline state is stored per recording in `outline-state.json`. Collapse all also collapses newly added nodes; individual overrides survive updates and restart. Existing heading-collapse preferences seed the new state on upgrade. Heading and parent-item IDs tolerate punctuation, case changes, and unrelated reordering. A substantially renamed label receives a new ID and uses the current default. Notes generation is instructed to retain labels. A pre-update scroll anchor keeps the current section in view when content above it changes.

Agents read grouped passages with `workspace/transcript`, visible nodes with `workspace/outline`, and persisted state with `workspace/outline_state`. `workspace/outline_set` uses the same validation, storage, and UI event as human controls. Visible outline operations require the recording to be open and loaded; invalid or stale node IDs return an error. `workspace/set` opens View or Speakers and awaits persisted panel changes. Schemas declare read/control permissions for all 75 actions across 14 tools.

Connections and the new-meeting screen include an eight-second setup check. `audio/setup_start`, `setup_status`, and `setup_cancel` expose the same operation. It unlocks saved connections and opens both capture inputs, discards all samples, and makes no provider request. It rejects concurrent recording or another setup check. Cancellation during a blocking macOS approval takes effect after that prompt returns. Keychain remains the persistent credential store; successfully unlocked keys are reused only in process memory, with serialized access to avoid simultaneous prompts. macOS still controls approval; this cannot guarantee that macOS never prompts again after an update or permission change.

Manual acceptance helper: `python3 tools/agent-control/tests/live_workspace.py exercise`, restart the idle app, then run the helper with `restart`. It requires the existing explicitly disposable fixture pointer, preserves workspace preferences, checks visible and persisted outline state, tests additions during refresh, rejects stale nodes, reads source-linked transcript paragraphs, and restores fixture notes. It does not start a paid transcription or edit real recordings. The evidence file is `target/partyline-043-acceptance.json`.

### 0.4.3 validation

Automated checks passed: 23 UI/controller/render tests, 19 Python bridge/inventory tests, 14 Rust permission/service tests, 18 meeting-library/model tests, three recording-resilience tests, and five native cloud tests. The production frontend build includes type checking. The transcript rendering test checks inline source fragments, keyboard playback, and disabled playback during capture; outline tests exercise real rendered parent controls and scroll-anchor restoration.

Live native acceptance passed for the new outline reads/updates, persistence through notes edits and restart, new-node collapse defaults, stale-node rejection, grouped transcript source ranges, panel switching, and workspace preferences. A focus-restoration bug found by switching View directly to Speakers was fixed and the transition passed in both directions. Human Collapse all matched agent state; dragging the transcript divider saved its new width and the original width was restored. Sticky speaker labels were observed while scrolling the hour-long saved livestream. The Ask pane opens alongside the workspace.

The setup check completed on first launch and again after a signed update with no new approval prompts: both inputs supplied samples, Mac audio had signal, and the microphone was silent. Cancellation completed without creating a recording. This is not a microphone speech test or a guarantee about future macOS prompts. Control access and the existing five-minute notes interval survived restart. Eight non-fixture recordings retained their audio/meeting file sizes and modification times throughout these tests. The app, installed copy, and DMG are checked for matching executable, metadata, signature resources, and bundled agent bridge/catalog.

Limits: live-follow growth and refresh scroll anchoring have deterministic component/controller tests; a new long real-time recording was not made for this UI release. The three-hour test injects accelerated PCM with stalled transcription rather than running hardware for three hours. Existing unsupported behaviors listed above remain unsupported; this release does not claim a fresh end-to-end run of every historical action.


## Record first — 0.4.4

Both the welcome screen and sidebar start recording immediately, using the selected microphone plus Mac audio. A blank title gets the existing local date/time default. The setup form is replaced by one primary action and an input selector; connection and permission checks remain in Connections. Agent-prepared title and participant suggestions remain visible on the welcome screen and are used when starting there. Sidebar Start recording starts a fresh, unnamed session.

Clicking the title opens the shared details editor during connecting, recording, or pause. `meetings/update` uses the same version check, input validation, persistence, and events. `workspace/set` with panel `details` opens that editor during capture, and `status/get` reports live participant editing supported. No voice identity is inferred from participants. Edits wait during finalization; destructive and audio edits still require an idle recording.

Live metadata edits clear the whole-meeting Undo pointer: restoring a live snapshot later would discard subsequent transcript or capture state. Revisions are still retained for audit. Saved-meeting edits retain normal Undo. Live details can save while running notes are streaming and do not schedule an audio backup until the recording is finished.

Calendar connection and cross-app meeting detection remain unimplemented. There is no calendar OAuth grant, event picker, meeting-app detector, or automatic recording trigger in this build. Proposed follow-up: optionally attach a calendar event after capture begins, then add a human-confirmed detected-meeting prompt. Invitee names remain suggestions.

### 0.4.4 validation

Automated checks passed: 24 UI/controller/render tests, 20 meeting-library/model tests, 14 Rust agent permission/service tests, and 19 Python bridge/inventory tests. Type checking, native compilation, the production bundle build, and inventory drift checks passed. New tests cover immediate start without metadata fields, live details during notes streaming, stale/invalid edits, finalization guards, and preventing Undo from restoring an obsolete live snapshot.

An 18.79-second disposable recording started through the welcome button. Agent title/participant edits and a human title edit saved during capture; stale and blank-title edits were rejected. The recording completed without errors, with matching microphone and Mac-audio WAV durations. Mac audio carried signal; the microphone supplied samples but was silent, so this was not a speech test. No new permission prompts appeared in this test. Evidence: `target/partyline-044-acceptance.json`; repeatable helper: `tools/agent-control/tests/live_quick_start.py` with an explicitly identified test recording.

The signed app is installed at `/Applications/PartyLine.app`. The installed executable, metadata, signature resources, and agent bridge/catalog match the app in `PartyLine_0.4.4_aarch64.dmg`. Control remains enabled. Eight pre-existing recordings retained their audio/meeting file sizes and modification times. The 0.4.2–0.4.5 source changes were committed in `d731cea`; GitHub agent-parity CI passed for that commit.

## Google Calendar and meeting detection — 0.4.5

PartyLine connects Google accounts directly with a dedicated Desktop app OAuth client, system-browser consent, PKCE, a random state value, and a loopback-only callback. Account IDs come from Google userinfo. Refresh tokens and the client secret stay in Keychain; neither results nor activity logs contain token/key values. Account email addresses and selected calendars are local settings. Events are cached in memory for the past 12 hours and next seven days, refreshed every five minutes and on request. Cancelled, declined, and all-day events are excluded. Sign-in, sync, per-account errors, and retry are visible through UI and tools.

Each account has its own calendar selection. A failing account does not block other accounts; its stale event suggestions are cleared. Attaching an event stores a snapshot with the recording. The optional title/invitee replacement uses the same validated live edit path as manual details; it never names a voice. Unlinking retains the current meeting details. Removing an account removes local credentials and event suggestions but preserves links already attached to recordings. It does not revoke Google's app grant; use Google Account permissions for that.

Detection is opt-in and checks microphone-using processes every five seconds on macOS 14.2+. Two observations are required before showing a prompt. Dismissal lasts until activity has been absent for six checks. PartyLine, music playback, and unrelated apps are excluded. Supported app families are Zoom, Teams, Slack, FaceTime, Webex, Chrome, Safari, Edge, and Firefox. Browser microphone activity is labeled as a possible call: no browser page, participant list, screenshot, or window title is inspected. A muted or listen-only call may not be detected. Capture never starts automatically; the prompt uses the existing Start recording action.

Unsupported: direct calendar writes, calendar-event creation/deletion, browser-tab identification, automatic speaker identity, silent account consent, and automatic recording. Google Workspace administrators may restrict the app. Google OAuth testing mode can expire offline grants after seven days; publishing/verification is a separate Google setup decision.

References: [Google desktop OAuth](https://developers.google.com/identity/protocols/oauth2/native-app), [Calendar read-only scopes](https://developers.google.com/workspace/calendar/api/auth), [Core Audio process input activity](https://developer.apple.com/documentation/coreaudio/kaudioprocesspropertyisrunninginput).

### 0.4.5 validation

Automated checks passed: 25 UI/controller/render tests, 19 Python bridge/inventory tests, 15 Rust agent permission/service tests, 24 meeting-library/model tests, six isolated calendar/OAuth tests, and five native calendar/OAuth/HTTP tests. The OAuth cases overlap between the isolated and native suites. Type checking, production compilation, and the 15-tool/87-action inventory check passed. Coverage includes callback state and PKCE, account separation, provider pagination/errors, event validation, live metadata edits, and permission enforcement.

The signed 0.4.5 app is installed at `/Applications/PartyLine.app`. Its executable, metadata, signature resources, and packaged bridge/catalog match the built app and `PartyLine_0.4.5_aarch64.dmg`. Eight pre-existing recordings retained their audio/meeting file sizes and modification times. Control remains enabled.

Native acceptance passed for opening Calendar through the agent, closing it through the UI and observing that state through the agent, rejecting invalid actions, running the process detector without an error, and restoring its original disabled setting. The agent-opened client JSON picker cancelled without changing the saved configuration. A dedicated Google Desktop client was created, Calendar API enabled, and its configuration saved through the app UI; the agent reports it configured without returning credentials.

The human completed consent for Personal, Stimulus, CareAtlas, and MFC. All four accounts synced without errors, with 72 timed events across their selected primary calendars. Account IDs and event IDs remained distinct. Deselecting one account cleared only its event suggestions; restoring the selection restored its events. A disposable recording passed attachment with and without title/invitee replacement, stale-version rejection, and unlinking while retaining details. Its original title/participants were restored and WAV hashes remained unchanged. Agent attachment appeared in the UI; human Unlink was reflected in the agent's meeting read.

After quitting the idle app and reopening `/Applications/PartyLine.app`, all four accounts, selected calendars, and Control access persisted. Calendar sync completed again with 72 events and no account errors. Meeting detection remains off until the human enables it.

Evidence: `target/partyline-045-calendar-acceptance.json`, `target/partyline-045-connected-acceptance.json`, `target/partyline-045-bundle-verification.json`, and `target/partyline-045-preservation-before.json`. Repeat the pre-consent checks with `python3 tools/agent-control/tests/live_calendar.py` while idle. `live_calendar_connected.py` requires at least two authorized accounts and the explicitly disposable fixture; it restores calendar selections and fixture details after testing. No new long recording, token-expiry refresh, account-removal/re-consent, or all-app detection acceptance was run for this release. The 0.4.2–0.4.5 source changes were committed in `d731cea`; GitHub agent-parity CI passed for that commit.


### 0.4.6 chat controls and cited sources

Ask recordings sends with Enter and inserts a newline with Shift+Enter. IME composition, held Enter, blank questions, and an active request cannot submit another answer. Keyboard and button submissions share the existing `cloud_chat_ask` path; the MCP `chat.ask` permission remains Control.

`cloud_chat_list` and MCP `chat.list` now return `cited_source_indices` for each reply: unique, one-based source numbers in answer order. The UI renders only those source links. Original source order, citation numbers, and playback positions remain intact, including saved answers. Full retrieval context remains available in `sources` for agent inspection. Listing remains Read; the native adapter and permission rules are unchanged.

Behavior checks cover actual composer callbacks, source link rendering and playback targets, and native citation selection. No new tool action or unsupported behavior was introduced.

Google setup on November 17, 2026: PartyLine's OAuth audience now shows **In production**. Homepage, privacy, and terms are published at `https://heychad.github.io/partyline/`; the repository is `heychad/partyline`. Google verification has not been completed; the console retains its unverified-app user cap. Production removes the testing-mode seven-day expiration for new offline grants. Whether existing testing-issued grants inherit that change was not verified; an existing account may need one final reconnect. No grants were revoked or permissions expanded during publication.

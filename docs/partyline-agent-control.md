# PartyLine agent control

Eight MCP tools cover 33 actions in the cloud workspace and its native commands. They call the same recording, speaker, notes, export, and Keychain functions as the app. A small view bridge handles setup fields, history selection, Connections, banners, and transcript following.

The implementation is in `frontend/src-tauri/src/agent_control/`, `frontend/src/components/agent-control/`, and `tools/agent-control/`. The macOS app and DMG rebuilt on November 14, 2026 include persistent access settings. All 33 actions have live acceptance coverage across the normal app and an isolated native key-test profile. New installations start with agent access Off.

## Parity inventory

Inventory source: `CloudWorkspace.tsx`, `cloud/mod.rs`, `cloud/keys.rs`, `lib.rs`, and `tray.rs` on the working cloud branch.

| Human action or visible state | Agent action | Existing implementation |
| --- | --- | --- |
| App status, active meeting, connection state | `status/get` | `cloud_current`, `cloud_key_status` |
| Open or hide the window; quit normally | `status/show`, `status/hide`, `status/quit` | Main window methods; existing exit handler drains capture and notes |
| List microphones | `audio/list` | `get_audio_devices` |
| Select the next microphone | `audio/select` or `recording/start.microphone` | Setup form; `cloud_start` |
| Live microphone/Mac meters, elapsed time, warnings | `audio/diagnostics` | Existing `cloud-levels` events and `cloud_current` |
| New meeting; title and participant suggestions | `meetings/new`, `meetings/prepare` or `recording/start` | Setup fields; `cloud_start` |
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
| Grant/revoke agent access; saved level across restarts | Human-only **Agents** control; `status/get` reports the level | Main-window permission commands and private access settings; agents cannot grant access |

Tool names have the `partyline_` prefix. For example, `notes/get` means `partyline_notes` with `{"action":"get","id":"…"}`. The machine-readable catalog is `frontend/src-tauri/src/agent_control/tools.json`; `catalog.py` generates it.

Unsupported actions are explicit in `status/get`: manual transcript-text edits, meeting deletion, changes to the live microphone or participant list, and update checks. The cloud UI does not implement the first four. The tray dispatches an update event, but the cloud root does not mount its listener. Legacy local-model screens and commands are outside this cloud API.

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

The bridge never retries a failed mutation automatically. A lost response can mean the action ran; inspect app state before retrying. Requests are limited to 64 KiB, responses to 16 MiB, and simultaneous socket exchanges to eight. Use transcript pagination for long meetings.

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

The maintenance review covers all 33 tool actions and all current public cloud commands. The November 14 update adds persistent human-controlled grants, matching UI/tool descriptions, and restart, revocation, invalid-file, and failed-save tests. Tool action names and required permission levels are unchanged.

## Verification

Run without launching PartyLine, reading its saved meetings, accessing Keychain, or calling a provider:

```sh
python3 tools/agent-control/check_inventory.py --base HEAD
cargo test --manifest-path tools/agent-control/Cargo.toml --locked --offline -j 1
python3 -m unittest discover -s tools/agent-control/tests -p 'test_*.py' -v
node --test tools/agent-control/tests/ui.test.mjs
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

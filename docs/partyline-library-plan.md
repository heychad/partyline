# Recording library implementation

Completed November 15, 2026: make saved calls editable, playable, shareable, searchable, and available to Codex and Claude, with the same behavior through PartyLine's agent tools.

## Scope

- Edit titles, participants, transcript turns, and notes. Detect stale edits; keep revisions for undo.
- Trim the start and end without changing the original audio. Redact selected transcript ranges from shared text and audio.
- Move recordings to a recoverable Trash and restore them.
- Click transcript timestamps to seek and listen. Provide pause, seek, channel selection, and playback speeds.
- Copy notes, transcript, or both; export text and audio with trims and redactions applied.
- Search saved calls and ask questions across recordings. Answers cite actual transcript passages and open the matching audio.
- Automatically back up finished recordings and edits through Google Drive for desktop. The user chooses the folder in Settings; nothing uploads before selection. Retain local originals and distinguish local copy completion from cloud upload.
- Configure the MCP bridge for Codex and Claude and provide repeatable setup and diagnostics.

## Implementation and acceptance

1. Build shared, testable meeting-editing, revision, redaction, search, and audio-export logic.
2. Connect native commands and the human UI to that logic. Add playback, sharing, Trash, and chat.
3. Extend the MCP catalog, native adapter, permissions, state/events, inventory, and behavioral tests with every feature.
4. Add Drive backup and client setup with visible progress, errors, and recovery.
5. Run automated checks, rebuild the app and DMG, and exercise the features through the UI and agent tools on disposable recordings.

Original recordings, API keys, and unrelated work must remain intact. Do not rebuild over an active recording. Trimmed/redacted exports must not contain excluded audio or transcript text. Notes derived before a redaction must not be shared as though they were reviewed. External messages are prepared or copied, never sent automatically.

## Status

- [x] Scope confirmed: text/details editing, start/end trims, and sharing redactions.
- [x] Shared library logic and tests
- [x] UI and audio playback, including live human/MCP checks
- [x] Agent routes and inventory; automated and local live behavior checks
- [x] Search and cited chat, including a completed live provider answer
- [x] Google Drive backup and automatic-edit acceptance; destination left for the user to choose
- [x] Packaged Codex and Claude MCP setup; all three clients configured
- [x] Final app/DMG build, signature/checksum verification, and matching packaged contents
- [x] Completed live AI answer; acceptance, cancellation, source playback, and invalidation passed

Acceptance: [agent-control inventory and verification](partyline-agent-control.md#library-acceptance). The app and DMG are built and verified. The Drive folder remains unset for the user to choose; automatic backups and agent Control remain enabled. The tested app is installed at `/Applications/PartyLine.app`.

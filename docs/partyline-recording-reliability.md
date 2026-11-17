# Recording reliability — 0.4.2

## Workshop incident, November 17, 2026

The 10:01 a.m. workshop recording stopped around 10:53 a.m. Both local WAV files contain 3,096.1 seconds (51:36). Their physical lengths exactly match their headers: there are no additional samples behind a stale header. The saved Mac-audio transcript has 292 completed turns and reaches 3,096.053 seconds. The microphone has signal but no live transcript turns; file transcription is the remaining recovery route for that channel. No same-day backup was found in the standard Audio Hijack or Zoom recording locations inspected.

The meeting recorded the exact error from the local capture queue's three-second overflow guard. The old code consumed 100 ms of audio per timer tick, used delayed missed-tick scheduling, and awaited shared UI/JSON publication on that same loop. Scheduling or publication delays could therefore accumulate instead of being drained. The error establishes the stop path; no retained application log establishes the precise scheduling event that filled the queue.

A copy of each original file and its SHA-256 hash is preserved in the meeting's `recovery-original-20261117` folder. No audio beyond the stop time has been found. Recovery cannot recreate speech that was never recorded.

## Changes

- Capture and WAV writes run on a dedicated runtime. Samples are written on arrival; a periodic timer only checkpoints files and reports health. Capture never awaits provider, Keychain, meeting-state, or UI work.
- Only the bounded live-transcription copy can be dropped. Local audio continues; diagnostics report dropped live seconds and a recovery-needed flag.
- Local audio starts while API keys unlock and providers connect. Transcription retries with backoff; sessions renew before AssemblyAI's three-hour limit. Separate speaker IDs avoid assigning a new session's voices to previous people without confirmation.
- Two-second header/data checkpoints reduce unclean-exit exposure. Header recovery preserves a backup before making additional flushed PCM readable.
- Saved-audio recovery uploads a selected original channel to AssemblyAI, saves job IDs, resumes polling on retry, exposes cancellation and errors, and retains a transcript revision for undo. It does not invent identities or silently replace the other channel. Notes require review afterward.
- Disk/device errors remain fatal when audio cannot be captured or saved. The app reports an interrupted recording and requests a desktop notification and attention. Notification display depends on macOS permissions.
- Existing automatic Drive-folder backup remains available once the user selects its destination. A completed local copy is not proof of cloud upload.

## Workshop interface

Speaker merges work during recording and on saved calls, preserve raw provider IDs and confirmed names, apply to subsequent turns, and support undo. Notes refresh can be set to 30 seconds, 1, 2 (default), or 5 minutes, or Manual. Refresh now and final notes remain available. Notes render as a collapsible topic outline with nested items. Column widths, sidebar visibility, transcript pinning, and outline collapse choices persist. Chat is docked, and new live transcript turns no longer invalidate unchanged citations.

All new actions use the same native validation, persistence, and events for human and agent controls. The inventory has 14 tools and 68 actions. Agent access remains human-controlled.

## Verification

The isolated recorder test writes three hours of 48 kHz PCM to both WAVs at accelerated speed. It verifies exact duration, final audio samples, a full-session transcription blackout, and a reconnect after two hours. Separate tests cover checkpoints, truncated-header recovery, and write failures. This is not a three-hour real-time hardware soak.

Shared-library tests cover recovery channel isolation and timestamp validation, merge persistence/future turns/undo, manual refresh and layout settings, and live citation preservation with invalidation after edits or redaction. Agent tests check permissions, schema validation, error propagation, and UI state callbacks. Live installed-app acceptance and workshop recovery results are recorded in `target/partyline-042-acceptance.json` when complete.

Limits: live input switching and hardware-device reconnection remain unsupported. No software can preserve new audio through a powered-off Mac, a disconnected source, or a full/unwritable disk. The WAV format supports more than three hours per channel at this sample rate; its approximately 12-hour file-size boundary has not been validated for continuous use. If a provider accepts a paid job but its acknowledgement is lost before the ID is saved, automatic resubmission could duplicate work, so the error asks for provider inspection before retrying.

References: [AssemblyAI session termination and three-hour limit](https://www.assemblyai.com/docs/streaming/message-sequence), [file transcription and speaker labels](https://www.assemblyai.com/docs/pre-recorded-audio/label-speakers).

# PartyLine cloud version

PartyLine sends two separate audio streams to AssemblyAI: your microphone is always **ME**, and Mac audio gets speaker labels. GPT-6.1 Terra turns completed transcript turns into running notes and a final summary.

## First recording

1. Open **Connections**. Enter AssemblyAI and OpenAI API keys, then choose **Save & test**. Keys are stored in macOS Keychain and never returned to the web interface. The test checks AssemblyAI authentication and Terra model access; it does not run a paid transcription or notes request.
2. Enter a meeting title. Optionally paste participant names from an invitation, separated by commas or newlines.
3. Use headphones, select your microphone, and choose **Start recording**. Allow microphone and Mac audio capture when macOS asks.
4. Speak, then play a short conversation on your Mac. Check both audio meters. Your words should appear under ME; the other voices should appear as speakers.
5. Click a speaker label above the transcript and confirm their name. The participant list supplies suggestions; it does not assign identities automatically.
6. Check the notes, then choose **Stop**. PartyLine drains final transcript and speaker corrections before generating the final notes.

Notes start after the first completed utterance. Further requests are spaced at least eight seconds apart, plus the time needed for the previous response. Text streams onto the screen as Terra returns it. Speaker corrections trigger a refresh. A failed request leaves the last successful notes intact.

**Files** opens the meeting folder. The copy button exports notes and transcript to `meeting.md` there and copies the text to the clipboard. Saved meetings can be reopened, speakers renamed, and notes refreshed.

## Audio and recovery

Each meeting is stored under `~/Library/Application Support/com.heychad.partyline/cloud-meetings/<meeting-id>/`:

- `meeting.json`: transcript, confirmed names, notes, and recording status.
- `microphone.wav` and `mac-audio.wav`: separate 48 kHz mono recordings. WAV headers are flushed every two seconds.
- `meeting.md`: created when exporting.

Audio is written locally before it enters a bounded network queue. Connection failures appear in the recording view. Reconnects use fresh speaker IDs so that a new “Speaker A” cannot inherit an earlier person’s name. If audio cannot be sent, the transcript may have gaps; local audio remains available. This version does not automatically retranscribe those gaps.

Pause stores and sends silence until resumed; it keeps the timeline and connections open. AssemblyAI continues billing an open session during a pause. Closing the window leaves recording active in the menu bar. Quitting stops capture and waits for final transcript and notes to be saved.

A failed audio device or disk write stops recording and displays an error. If the process is force-quit, the next launch marks that meeting interrupted. The most recent audio header update or transcript save may be missing.

## Speaker names

There is no fixed expected participant count. Remote voices appear as they are detected, up to the provider’s streaming diarization limit. Short utterances can remain unidentified, overlapping voices can be misassigned, and speaker labels can change as more speech arrives. Review consequential ownership or action items.

Mac audio capture excludes Rogue Amoeba routing processes and eqMac, including virtual microphone outputs. Otherwise, a processed microphone can appear both as ME and as a remote speaker even with monitoring off. The filter is applied when a recording starts; rebuilding the app does not change an active recording.

A list of invitees only tells us who might be present. Screenshots and active-speaker indicators could help suggest a voice/name match later, but this version uses confirmation in the interface. No browser, calendar, or screenshot access is required.

## Provider configuration

- [AssemblyAI streaming](https://www.assemblyai.com/docs/streaming/api-spec/streaming-websocket): `universal-3-6-pro`, balanced mode, continuous partials, 100 ms PCM frames. Remote diarization requests revisions every two minutes and receives final revisions at termination. No expected speaker count is sent.
- [Speaker labels](https://www.assemblyai.com/docs/streaming/label-speakers-and-separate-channels): microphone and Mac audio use separate sessions. Word-level speaker changes are preserved.
- [GPT-6.1 Terra](https://developers.openai.com/api/docs/models/gpt-6.1-terra): Responses API, `store: false`, streaming Markdown, no reasoning for live notes and low reasoning for final notes. Live updates use existing notes and new transcript when possible; corrected transcripts and final summaries use the full transcript.

No local model downloads are required. Earlier local recordings and settings are left on disk. The cloud history lists recordings made with this version.

## Verification

Automated coverage includes partial replacement, word-level speaker revisions, confirmed names, reconnect identity separation, PCM conversion, UTF-8 SSE fragmentation, incomplete notes responses, and a local mock WebSocket test for authentication, audio frames, and termination draining. The first live check on November 13, 2026 confirmed AssemblyAI transcription from Mac audio and the microphone, plus Terra notes. It also exposed duplicated microphone audio from the virtual routing chain. The routing exclusion compiles and passes its regression test; its live verification requires restarting after the active meeting.

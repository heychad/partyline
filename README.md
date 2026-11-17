# PartyLine

A personal macOS meeting recorder with live transcription, notes, and agent controls. Built by Chad Owen from [Meetily](https://github.com/Zackriya-Solutions/meetily).

[Website](https://heychad.github.io/partyline/) · [Privacy](https://heychad.github.io/partyline/privacy.html) · [Terms](https://heychad.github.io/partyline/terms.html)

## What it does

- Records microphone and Mac audio to local files independently of cloud transcription.
- Uses AssemblyAI for transcription and diarization, and OpenAI for notes and recording chat.
- Supports speaker corrections, editable meeting details, audio playback, trimming, sharing redactions, and recoverable Trash.
- Connects multiple Google accounts with read-only calendar access and optional call-app detection.
- Offers Google Drive for desktop backups to a folder you choose.
- Exposes recording and library controls through a permissioned local MCP connection.

Recording starts when you choose. Google invitees are name suggestions, not verified speaker identities. Cloud features use your own provider connections.

## Build on macOS

Install the repository's Rust, Node, pnpm, and native audio dependencies, then:

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend tauri build --bundles app,dmg
```

The local macOS configuration names Chad's Apple Development signing identity. Use your own identity for a signed build. API keys and Google OAuth credentials belong in the app, not in source files.

The bundle is `PartyLine.app`; the internal Cargo package and executable retain the upstream `meetily` name. Current validation targets macOS; inherited cross-platform code is not a claim of Windows or Linux acceptance for these features.

## Development and validation

Read [AGENTS.md](AGENTS.md). Human and agent actions share validation, persistence, and events. New behavior must include its agent equivalent and tests.

- [Agent inventory, tests, and live acceptance](docs/partyline-agent-control.md)
- [Recording reliability and recovery limits](docs/partyline-recording-reliability.md)
- [Inherited Meetily documentation](README.upstream.md)

The three-hour recording test uses accelerated PCM. It is not a three-hour hardware soak. See the validation documents for what has and has not been tested.

## License

[MIT](LICENSE.md). Upstream copyright and attribution are retained.

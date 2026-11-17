"""Read-only acceptance for cited-source UI/MCP parity. Requires PartyLine 0.4.6.
Run with agent Read or Control access; does not submit questions or play audio.
"""
import json
import re
from live_workspace import ROOT, call


def run():
    status = call('status', {'action': 'get'})
    assert status['version'] == '0.4.6'
    replies = call('chat', {'action': 'list'})['items']
    counts = []
    for reply in replies:
        expected = list(dict.fromkeys(int(n) for n in re.findall(r'\[([0-9]+)\]', reply['answer']) if 0 < int(n) <= len(reply['sources'])))
        assert reply['cited_source_indices'] == expected
        counts.append({'retrieved': len(reply['sources']), 'displayed': len(expected), 'indices': expected})
    report = {'version': status['version'], 'access': status['agent_access'], 'replies': counts}
    (ROOT / 'target/partyline-046-chat-acceptance.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    run()

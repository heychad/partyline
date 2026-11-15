import importlib.util
import json
from pathlib import Path
import stat
import tempfile
import unittest
spec = importlib.util.spec_from_file_location('partyline_setup', Path(__file__).parents[1] / 'setup.py')
setup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(setup)

class SetupTests(unittest.TestCase):
    def test_desktop_merge_preserves_other_clients_and_is_idempotent(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'config.json'
            original = {'theme':'dark','mcpServers':{'unrelated':{'command':'keep-me','env':{'EXAMPLE':'do-not-print'}}}}
            path.write_text(json.dumps(original))
            self.assertTrue(setup.merge_desktop(path, Path('/bridge/server.py'), '/python'))
            result = json.loads(path.read_text())
            self.assertEqual(result['mcpServers']['unrelated'],original['mcpServers']['unrelated'])
            self.assertEqual(result['theme'],'dark')
            self.assertFalse(setup.merge_desktop(path, Path('/bridge/server.py'), '/python'))
            self.assertEqual(stat.S_IMODE(path.stat().st_mode),0o600)
            backup = path.with_name(path.name + '.partyline-backup')
            self.assertEqual(json.loads(backup.read_text()),original)
            self.assertEqual(stat.S_IMODE(backup.stat().st_mode),0o600)
    def test_invalid_existing_config_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'config.json';path.write_text('invalid')
            with self.assertRaises(ValueError): setup.merge_desktop(path, Path('/bridge/server.py'), '/python')
            self.assertEqual(path.read_text(),'invalid')
    def test_claude_code_repeat_setup_accepts_only_the_same_bridge(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'config.json'
            self.assertFalse(setup.claude_code_configured(path,Path('/bridge/server.py'),'/python'))
            config={'type':'stdio','command':'/python','args':['/bridge/server.py'],'env':{}}
            path.write_text(json.dumps({'mcpServers':{'partyline':config}}))
            self.assertTrue(setup.claude_code_configured(path,Path('/bridge/server.py'),'/python'))
            self.assertFalse(setup.claude_code_configured(path,Path('/different/server.py'),'/python'))
            config['env']={'OVERRIDE':'keep-me'}
            path.write_text(json.dumps({'mcpServers':{'partyline':config}}))
            self.assertFalse(setup.claude_code_configured(path,Path('/bridge/server.py'),'/python'))

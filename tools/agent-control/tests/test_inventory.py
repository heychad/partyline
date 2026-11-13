import copy
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import catalog
import check_inventory as inventory


class InventoryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='partyline-inventory-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        paths = [inventory.INVENTORY, inventory.NATIVE,
                 'frontend/src-tauri/src/agent_control/core.rs',
                 'frontend/src-tauri/src/agent_control/tools.json']
        paths += [str(p.relative_to(catalog.ROOT)) for p in (catalog.ROOT / 'frontend/src-tauri/src/cloud').rglob('*.rs')]
        for name in paths:
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(catalog.ROOT / name, destination)

    def test_current_inventory_matches(self):
        self.assertEqual(inventory.check(self.root, catalog.TOOLS), [])

    def test_new_ui_tool_requires_inventory_and_adapter(self):
        tools = copy.deepcopy(catalog.TOOLS)
        tool = tools[0]
        schema = copy.deepcopy(tool['inputSchema']['oneOf'][0])
        schema['properties']['action']['const'] = 'new_action'
        tool['inputSchema']['oneOf'].append(schema)
        (self.root / 'frontend/src-tauri/src/agent_control/tools.json').write_text(json.dumps(tools))
        errors = inventory.check(self.root, tools)
        self.assertIn('Undocumented agent action: status/new_action', errors)
        self.assertIn('Agent action has no native/service route: status/new_action', errors)

    def test_new_cloud_command_requires_both_mapping_and_adapter(self):
        (self.root / 'frontend/src-tauri/src/cloud/new.rs').write_text('#[tauri::command]\npub async fn cloud_recover() {}')
        errors = inventory.check(self.root, catalog.TOOLS)
        self.assertIn('Cloud command missing from inventory: cloud_recover', errors)
        self.assertIn('Cloud command missing from native adapter: cloud_recover', errors)

    def test_removed_action_and_changed_permission_are_detected(self):
        tools = copy.deepcopy(catalog.TOOLS)
        tools[0]['inputSchema']['oneOf'] = tools[0]['inputSchema']['oneOf'][1:]
        errors = inventory.check(self.root, tools)
        self.assertIn('Stale inventory action: status/get', errors)
        tools = copy.deepcopy(catalog.TOOLS)
        tools[0]['_meta']['partyline/permissions']['get'] = 'control'
        self.assertTrue(any('permissions drifted' in e for e in inventory.check(self.root, tools)))

    def test_source_changes_require_review_but_tests_and_styles_do_not(self):
        for path in ['frontend/src/components/new/Action.tsx', 'frontend/src-tauri/src/new.rs',
                     'frontend/src-tauri/permissions/new.json', 'tools/agent-control/catalog.py']:
            self.assertTrue(inventory.review_errors({path}))
            self.assertEqual(inventory.review_errors({path, inventory.INVENTORY}), [])
        self.assertEqual(inventory.review_errors({'frontend/tests/new.test.ts', 'frontend/src/app/globals.css'}), [])


if __name__ == '__main__':
    unittest.main()

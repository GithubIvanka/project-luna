#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location('desktop',Path(__file__).with_name('test-canonical-luna-bundle-launch.py'))
desktop=importlib.util.module_from_spec(spec);spec.loader.exec_module(desktop)
class Keyboard(unittest.TestCase):
    def test_hidden_trash_path(self):
        keys=desktop.text_keys('/home/.local/share/Trash/files')
        self.assertIn('dot',keys);self.assertIn('shift-t',keys)
        self.assertNotIn('.',keys);self.assertNotIn('T',keys)
    def test_shell_symbols_and_uppercase(self):
        self.assertEqual(desktop.text_keys('A_="x" > /a.b;'),['shift-a','shift-minus','equal','shift-apostrophe','x','shift-apostrophe','spc','shift-dot','spc','slash','a','dot','b','semicolon'])
    def test_unsupported_text_fails_explicitly(self):
        with self.assertRaises(ValueError):desktop.text_keys('путь')
if __name__=='__main__':unittest.main()

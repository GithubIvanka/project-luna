#!/usr/bin/env python3
"""Focused source-boundary checks for desktop resource packaging."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('packager',Path(__file__).with_name('prepare-luna-desktop-bundles.py'))
packager=importlib.util.module_from_spec(spec);spec.loader.exec_module(packager)

class SourceBoundary(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='luna-desktop-source-')
        self.root=Path(self.temp.name)
        (self.root/'lib').mkdir();(self.root/'usr/lib').mkdir(parents=True)
        (self.root/'usr/lib/libactual.so').write_bytes(b'pinned')
    def tearDown(self): self.temp.cleanup()
    def test_guest_absolute_link_stays_in_selected_root(self):
        (self.root/'lib/library.so').symlink_to('/usr/lib/libactual.so')
        self.assertEqual(packager.resolve(self.root,'lib/library.so'),self.root/'usr/lib/libactual.so')
    def test_relative_link_stays_in_selected_root(self):
        (self.root/'lib/library.so').symlink_to('../usr/lib/libactual.so')
        self.assertEqual(packager.resolve(self.root,'lib/library.so').read_bytes(),b'pinned')
    def test_parent_escape_is_rejected(self):
        (self.root/'lib/library.so').symlink_to('../../outside')
        with self.assertRaises(ValueError): packager.resolve(self.root,'lib/library.so')
    def test_cycle_is_rejected(self):
        (self.root/'lib/a').symlink_to('b');(self.root/'lib/b').symlink_to('a')
        with self.assertRaises(ValueError): packager.resolve(self.root,'lib/a')
    def test_directory_is_not_a_library(self):
        with self.assertRaises(ValueError): packager.resolve(self.root,'lib')
    def test_missing_file_is_not_host_fallback(self):
        with self.assertRaises(ValueError): packager.resolve(self.root,'usr/lib/libc.so.6')
    def native_receipt(self):
        runtime=self.root/'sdk';(runtime/'lib/apk/db').mkdir(parents=True)
        (runtime/'lib/apk/db/installed').write_bytes(b'pinned SDK')
        ghostty=self.root/'ghostty';(ghostty/'bin').mkdir(parents=True)
        files=self.root/'files';files.write_bytes(b'Files')
        yazi=self.root/'yazi';yazi.mkdir()
        patch=self.root/'patch';patch.write_bytes(b'reviewed patch')
        paths={'ghostty':ghostty/'bin/ghostty','luna-files':files,'yazi':yazi/'yazi','ya':yazi/'ya'}
        for name,path in paths.items(): path.write_bytes(name.encode())
        receipt={'format':1,'sdk_image_id':'sha256:'+'1'*64,
            'ghostty_revision':'332b2aefc6e72d363aa93ab6ecfc86eeeeb5ed28',
            'yazi_revision':'8dd895c695a5950330c2623eb43debf323b60654',
            'artifacts':{n:packager.digest(p) for n,p in paths.items()},
            'luna_files_sources':packager.files_source_hashes(),
            'ghostty_patch_sha256':packager.digest(patch),
            'sdk_package_database_sha256':packager.digest(runtime/'lib/apk/db/installed')}
        return receipt,runtime,ghostty,files,yazi,patch
    def test_matching_native_build_receipt_is_accepted(self):
        packager.verify_native_build(*self.native_receipt())
    def test_stale_native_binary_is_rejected(self):
        args=self.native_receipt();(args[2]/'bin/ghostty').write_bytes(b'old binary')
        with self.assertRaises(ValueError): packager.verify_native_build(*args)
    def test_missing_files_source_receipt_is_rejected(self):
        args=self.native_receipt();args[0].pop('luna_files_sources')
        with self.assertRaises(ValueError): packager.verify_native_build(*args)
    def test_stale_files_source_receipt_is_rejected(self):
        args=self.native_receipt();args[0]['luna_files_sources']['Cargo.lock']='0'*64
        with self.assertRaises(ValueError): packager.verify_native_build(*args)
    def test_changed_patch_is_rejected(self):
        args=self.native_receipt();args[-1].write_bytes(b'changed patch')
        with self.assertRaises(ValueError): packager.verify_native_build(*args)
    def test_changed_sdk_is_rejected(self):
        args=self.native_receipt();(args[1]/'lib/apk/db/installed').write_bytes(b'other SDK')
        with self.assertRaises(ValueError): packager.verify_native_build(*args)
    def failed_pack(self, keep=False):
        import json
        receipt,runtime,ghostty,files,yazi,_=self.native_receipt()
        (ghostty.parent/'build-provenance.json').write_text(json.dumps(receipt))
        inputs=self.root/'inputs';inputs.mkdir();(inputs/'inputs.lock.json').write_text('{}')
        output=self.root/'output'
        argv=['packager','--inputs',str(inputs),'--runtime-root',str(runtime),
            '--ghostty-prefix',str(ghostty),'--luna-files',str(files),'--yazi-prefix',str(yazi),
            '--cli',str(files),'--output',str(output)]
        if keep: argv.append('--keep-payload')
        with patch('sys.argv',argv), patch.object(packager,'verify_native_build'), patch.object(packager.subprocess,'run'), patch.object(packager,'command',side_effect=RuntimeError('readelf failed')):
            with self.assertRaisesRegex(RuntimeError,'readelf failed'): packager.main()
        return output
    def test_failed_pack_cleans_its_owned_payload(self):
        output=self.failed_pack()
        self.assertFalse((output/'luna-files-source/payload').exists())
        self.assertEqual((self.root/'usr/lib/libactual.so').read_bytes(),b'pinned')
    def test_failed_pack_retains_payload_only_when_requested(self):
        output=self.failed_pack(keep=True)
        self.assertTrue((output/'luna-files-source/payload/bin/luna-files').is_file())

    def test_menu_close_requires_matching_instance_pid_clean_exit_and_cleanup(self):
        spec=importlib.util.spec_from_file_location('menu_harness',Path(__file__).with_name('test-canonical-luna-bundle-launch.py'))
        harness=importlib.util.module_from_spec(spec);spec.loader.exec_module(harness)
        window='LUNA-DESKTOP: Files menu PID=321\n'
        outcome='application complete application=org.luna.files instance=17 pid=321 exit=Some(0) signal=None cleanup_ok=true\n'
        harness.verify_files_menu_exit(window+outcome)
        for text in [window,window+outcome.replace('pid=321','pid=322'),window+outcome.replace('exit=Some(0) signal=None','exit=None signal=Some(11)'),window+outcome.replace('cleanup_ok=true','cleanup_ok=false')]:
            with self.assertRaises(RuntimeError):harness.verify_files_menu_exit(text)

if __name__=='__main__': unittest.main()

#!/usr/bin/env python3
"""Focused safety and rotation checks for the generated development workbench."""
import argparse
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

REPO = Path(__file__).resolve().parent.parent


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, REPO / "tools" / filename)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


artifacts = module("artifacts", "canonical-luna-artifacts.py")
workbench = module("workbench", "canonical-luna-workbench.py")
MARKER = "Project Luna generated canonical output v1\n"
STEPS = "inputs input-negative-checks artifact-checks state-runtime app-runtime init build structure production display bundle-launch login-1 login-2 login-3 logout reboot shutdown".split()


class ArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="canonical-artifact-tests-", dir=REPO / "target")
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def marked(self, directory):
        directory.mkdir(parents=True)
        (directory / ".luna-canonical-output-v1").write_text(MARKER)
        return directory

    def results(self, directory):
        directory.mkdir(exist_ok=True)
        path = directory / "results.txt"
        path.write_text("\n".join(f"{name} PASS" for name in STEPS) + "\nCanonical Luna regression: PASS\n")
        return path

    def test_cleanup_only_generated_subtrees(self):
        output = self.marked(self.root / "output")
        for name in (".build", "luna-sys", "luna-data"):
            (output / name).mkdir()
            (output / name / "generated").write_text("cache")
        for name in ("luna-0.1.0.img", "source.rs", "results.txt"):
            (output / name).write_text("keep")
        artifacts.cleanup_build(output)
        self.assertEqual({p.name for p in output.iterdir()}, {".luna-canonical-output-v1", "luna-0.1.0.img", "source.rs", "results.txt"})

    def test_cleanup_handles_immutable_packages_without_following_aliases(self):
        output = self.marked(self.root / "output")
        package = output / ".build/payload/application.lunaapp/payload"
        package.mkdir(parents=True)
        (package / "program").write_text("immutable payload")
        external = self.root / "user-files"
        external.mkdir(); external.chmod(0o500)
        (package / "external").symlink_to(external, target_is_directory=True)
        for directory in [package, package.parent, package.parent.parent]:
            directory.chmod(0o555)
        try:
            artifacts.cleanup_build(output)
            self.assertFalse((output / ".build").exists())
            self.assertEqual(external.stat().st_mode & 0o777, 0o500)
            self.assertTrue(external.exists())
        finally:
            external.chmod(0o700)

    def test_cleanup_pins_directory_before_chmod(self):
        output = self.marked(self.root / "output")
        package = output / ".build/package"
        package.mkdir(parents=True)
        (package / "program").write_text("immutable payload")
        package.chmod(0o555)
        inode = package.stat().st_ino
        external = self.root / "user-files"
        external.mkdir(); external.chmod(0o500)
        chmod = os.fchmod
        swapped = False
        def replace_after_open(descriptor, mode):
            nonlocal swapped
            if not swapped and os.fstat(descriptor).st_ino == inode:
                swapped = True
                package.rename(package.with_name("pinned-package"))
                package.symlink_to(external, target_is_directory=True)
            chmod(descriptor, mode)
        try:
            with patch.object(artifacts.os, "fchmod", side_effect=replace_after_open):
                artifacts.cleanup_build(output)
            self.assertTrue(swapped)
            self.assertEqual(external.stat().st_mode & 0o777, 0o500)
            self.assertFalse((output / ".build").exists())
        finally:
            external.chmod(0o700)

    def test_cleanup_refuses_unmarked_navigation_and_symlinks(self):
        output = self.root / "unmarked"; output.mkdir()
        with self.assertRaises(ValueError): artifacts.cleanup_build(output)
        with self.assertRaises(ValueError): artifacts.cleanup_build(self.root / ".." / "user-work")
        marked = self.marked(self.root / "marked")
        link = self.root / "link"; link.symlink_to(marked, target_is_directory=True)
        with self.assertRaises(ValueError): artifacts.cleanup_build(link)
        (marked / ".build").symlink_to(output, target_is_directory=True)
        with self.assertRaises(ValueError): artifacts.cleanup_build(marked)
        self.assertTrue(output.exists())

    def test_regular_refuses_device_leaf_and_parent_symlinks(self):
        image = self.root / "image"; image.write_bytes(b"reference")
        link = self.root / "alias"; link.symlink_to(image)
        with self.assertRaises(ValueError): artifacts.regular(link)
        parent = self.root / "alias-root"; parent.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError): artifacts.regular(parent / "image")
        with self.assertRaises(ValueError): artifacts.regular("/dev/null")

    def test_overlay_is_small_and_preserves_reference(self):
        image = self.root / "source.img"
        with image.open("wb") as out: out.truncate(64 * 1024 * 1024)
        before = artifacts.digest(image)
        overlay = self.root / "guest.qcow2"
        artifacts.create_overlay(image, overlay)
        info = json.loads(subprocess.check_output(["qemu-img", "info", "--output=json", str(overlay)]))
        self.assertEqual(info["backing-filename"], str(image))
        self.assertEqual(info["backing-filename-format"], "raw")
        self.assertLess(overlay.stat().st_blocks * 512, 1024 * 1024)
        self.assertEqual(artifacts.digest(image), before)
        with self.assertRaises(ValueError): artifacts.create_overlay(image, overlay)

    def test_overlay_patch_writes_real_bytes_with_spaces_in_host_paths(self):
        work = self.root / "scratch with spaces"; work.mkdir()
        image = work / "source with spaces.img"
        with image.open("wb") as out: out.truncate(1024 * 1024)
        before = artifacts.digest(image)
        overlay = artifacts.create_overlay(image, work / "guest with spaces.qcow2")
        artifacts.write_overlay_block(overlay, 65536, b"X" * 65536, work)
        artifacts.run("qemu-io", "-f", "qcow2", "-c", "read -P 88 65536 65536", overlay)
        self.assertEqual(artifacts.digest(image), before)

    def test_lifecycle_injection_preserves_shell_quoting_and_fails_missing_marker(self):
        marker = '    echo "luna-graphical-session: starting Niri" >&2\n'
        template = "#!/bin/sh\nexec sh -c '\n" + marker + "wait\n'\n"
        for action in ("logout", "reboot", "shutdown"):
            result = artifacts.lifecycle_session(template, action)
            subprocess.run(["sh", "-n"], input=result, text=True, check=True)
            self.assertIn(f"/usr/bin/noctalia msg session {action}", result)
            self.assertIn(f"persisted-{action}", result)
        with self.assertRaises(ValueError): artifacts.lifecycle_session("no marker", "logout")
        with self.assertRaises(ValueError): artifacts.lifecycle_session(template + marker, "logout")

    def test_lifecycle_selects_live_noctalia_wayland_pair(self):
        runtime = self.root / "runtime"; runtime.mkdir()
        home = self.root / "home"; home.mkdir()
        commands = self.root / "commands"; commands.mkdir()
        (commands / "sleep").write_text("#!/bin/sh\nexit 0\n"); (commands / "sleep").chmod(0o755)
        (commands / "noctalia").write_text('#!/bin/sh\nprintf "%s\\n" "$WAYLAND_DISPLAY" > "$LUNA_TEST_DISPLAY"\n'); (commands / "noctalia").chmod(0o755)
        marker = '    echo "luna-graphical-session: starting Niri" >&2\n'
        template = "#!/bin/sh\nexec sh -c '\n" + marker + "wait\n'\n"
        text = artifacts.lifecycle_session(template, "logout").replace("/usr/bin/noctalia", str(commands / "noctalia"))
        sockets = []
        try:
            for name in ("wayland-0", "wayland-1", "noctalia-wayland-1.sock"):
                item = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                sockets.append(item); item.bind(str(runtime / name))
            env = os.environ.copy()
            result = self.root / "selected-display"
            env.update(HOME=str(home), XDG_CONFIG_HOME=str(home / ".config"), XDG_RUNTIME_DIR=str(runtime), WAYLAND_DISPLAY="wrong-inherited-value", LUNA_TEST_DISPLAY=str(result), PATH=str(commands) + ":" + env["PATH"])
            subprocess.run(["sh"], input=text, env=env, text=True, check=True, capture_output=True, timeout=5)
            self.assertEqual(result.read_text(), "wayland-1\n")
        finally:
            for item in sockets: item.close()

    def test_standalone_patch_refuses_in_place(self):
        image = self.root / "image"; image.write_bytes(b"reference")
        with self.assertRaisesRegex(ValueError, "locked workbench"):
            artifacts.patch_system_image(image, [])

    def test_lifecycle_setup_failure_removes_its_overlay(self):
        image = self.root / "invalid-gpt.img"
        with image.open("wb") as out: out.truncate(64 * 1024 * 1024)
        case = self.root / "case"
        env = os.environ.copy()
        env.update(LUNA_CANONICAL_IMAGE=str(image), LUNA_CANONICAL_TEST_ROOT=str(case), LUNA_CANONICAL_KEEP_TEST_DISKS="0")
        result = subprocess.run(["bash", str(REPO / "tools/test-canonical-luna-lifecycle-ovmf.sh"), "logout"], env=env, capture_output=True, text=True, timeout=15)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((case / "guest.qcow2").exists())
        self.assertTrue((case / "logs/patch.json").exists())
        self.assertTrue(image.is_file())

    def test_login_setup_failure_cleans_overlay_and_preserves_existing_file(self):
        image = self.root / "source.img"
        with image.open("wb") as out: out.truncate(64 * 1024 * 1024)
        commands = self.root / "commands"; commands.mkdir()
        stub = commands / "cp"; stub.write_text("#!/bin/sh\nexit 1\n"); stub.chmod(0o755)
        case = self.root / "case"
        env = os.environ.copy()
        env.update(LUNA_CANONICAL_IMAGE=str(image), LUNA_CANONICAL_TEST_ROOT=str(case), LUNA_CANONICAL_KEEP_TEST_DISKS="0", PATH=str(commands) + ":" + env["PATH"])
        script = ["bash", str(REPO / "tools/test-canonical-luna-login-ovmf.sh")]
        result = subprocess.run(script, env=env, capture_output=True, text=True, timeout=15)
        self.assertNotEqual(result.returncode, 0)
        overlay = case / "canonical-login-test.qcow2"
        self.assertFalse(overlay.exists())
        overlay.write_text("existing user diagnostic")
        result = subprocess.run(script, env=env, capture_output=True, text=True, timeout=15)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(overlay.read_text(), "existing user diagnostic")

    def test_unexpected_successful_qemu_exit_fails_production_check(self):
        image = self.root / "image"; image.write_bytes(b"test")
        commands = self.root / "commands"; commands.mkdir()
        stub = commands / "qemu-system-x86_64"; stub.write_text("#!/bin/sh\nexit 0\n"); stub.chmod(0o755)
        env = os.environ.copy()
        env.update(LUNA_CANONICAL_IMAGE=str(image), LUNA_CANONICAL_TEST_ROOT=str(self.root / "case"), PATH=str(commands) + ":" + env["PATH"])
        result = subprocess.run(["bash", str(REPO / "tools/test-canonical-luna-production-ovmf.sh")], env=env, capture_output=True, text=True, timeout=15)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unexpected QEMU exit: 0", result.stderr)

    def test_atomic_state_ignores_stale_tmp_symlink(self):
        original = self.root / "user-file"; original.write_text("keep")
        (self.root / "state.tmp").symlink_to(original)
        workbench.atomic_json(self.root / "state.json", {"reference": "a"})
        self.assertEqual(original.read_text(), "keep")
        self.assertFalse((self.root / "state.json").is_symlink())
        self.assertEqual(json.loads((self.root / "state.json").read_text()), {"reference": "a"})

    def test_prune_only_owned_old_test_overlays(self):
        root = self.root / "managed"; root.mkdir()
        evidence = root / "evidence"; evidence.mkdir()
        for name in ("20261011-000000-000001", "20261011-000000-000002"):
            case = evidence / name / "logout"; case.mkdir(parents=True)
            (case / "guest.qcow2").write_text("temporary")
            (case / "serial.log").write_text("keep evidence")
        owned = evidence / "20261011-000000-000001"
        (owned / ".luna-canonical-evidence-v1").write_text("Project Luna generated canonical evidence v1\n")
        (owned / "logout/user-harness.py").write_text("keep source")
        with patch.object(workbench, "ROOT", root): workbench.prune_test_disks()
        self.assertFalse((owned / "logout/guest.qcow2").exists())
        self.assertEqual((owned / "logout/serial.log").read_text(), "keep evidence")
        self.assertEqual((owned / "logout/user-harness.py").read_text(), "keep source")
        self.assertTrue((evidence / "20261011-000000-000002/logout/guest.qcow2").exists())

    def test_prune_refuses_symlink_to_user_image(self):
        root = self.root / "managed"; root.mkdir()
        run = root / "evidence/20261011-000000-000001"; run.mkdir(parents=True)
        (run / ".luna-canonical-evidence-v1").write_text("Project Luna generated canonical evidence v1\n")
        (run / "logout").mkdir()
        image = self.root / "user.img"; image.write_text("keep")
        (run / "logout/guest.qcow2").symlink_to(image)
        with patch.object(workbench, "ROOT", root):
            with self.assertRaises(ValueError): workbench.prune_test_disks()
        self.assertEqual(image.read_text(), "keep")

    def initialize(self, root, inputs):
        image = self.root / "accepted.img"; image.write_bytes(b"accepted reference")
        workbench.checksum(image)
        results = self.results(self.root / "reference-evidence")
        with patch.object(workbench, "ROOT", root), patch.object(sys, "argv", ["workbench", "init", "--image", str(image), "--results", str(results), "--inputs", str(inputs)]), patch.object(signal, "signal"):
            workbench.main()

    def test_initialization_validates_before_copy_and_recovers_owned_import(self):
        root, inputs = self.root / "managed", self.root / "inputs"
        with self.assertRaisesRegex(ValueError, "input snapshot"): self.initialize(root, inputs)
        self.assertFalse((root / "slots/a").exists())
        inputs.mkdir(); (inputs / "inputs.lock.json").write_text("{}")
        interrupted = self.marked(root / "slots/a")
        (interrupted / workbench.IMAGE).write_bytes(b"partial")
        self.initialize(root, inputs)
        self.assertEqual((root / "slots/a" / workbench.IMAGE).read_bytes(), b"accepted reference")
        self.assertEqual(json.loads((root / "state.json").read_text())["reference"], "a")

    def test_initialization_never_adopts_unknown_import_files(self):
        root, inputs = self.root / "managed", self.root / "inputs"
        inputs.mkdir(); (inputs / "inputs.lock.json").write_text("{}")
        root.mkdir(); (root / ".luna-workbench-v1").write_text("Project Luna generated development artifacts v1\n")
        interrupted = self.marked(root / "slots/a")
        (interrupted / "user-file").write_text("preserve")
        with self.assertRaisesRegex(ValueError, "unknown interrupted"): self.initialize(root, inputs)
        self.assertEqual((interrupted / "user-file").read_text(), "preserve")

    def test_acceptance_requires_every_step_and_final_pass(self):
        results = self.results(self.root / "evidence")
        self.assertTrue(workbench.acceptance_complete(results))
        content = results.read_text()
        for invalid in (content.replace("reboot PASS\n", ""), content + "other FAIL\n", content.replace("shutdown PASS", "shutdown FAIL")):
            results.write_text(invalid)
            self.assertFalse(workbench.acceptance_complete(results))
        results.write_text(content.replace("artifact-checks PASS\n", ""))
        self.assertTrue(workbench.acceptance_complete(results))  # Import older accepted source images.
        self.assertFalse(workbench.acceptance_complete(results, workbench_checks=True))
        for step in ("app-runtime", "bundle-launch"):
            results.write_text(content.replace(f"{step} PASS\n", ""))
            self.assertFalse(workbench.acceptance_complete(results, workbench_checks=True))

    def test_test_fixture_requires_disposable_overlay(self):
        image = self.root / "image.img"; image.write_bytes(b"preserve")
        with self.assertRaisesRegex(ValueError, "disposable overlay"):
            artifacts.patch_system_image(image, [], prepare_test_tree=lambda tree: None)
        self.assertEqual(image.read_bytes(), b"preserve")

    def test_promotion_swaps_roles_and_keeps_both_images(self):
        root = self.root / "workbench"; root.mkdir()
        for name in ("a", "b"):
            directory = self.marked(root / "slots" / name)
            (directory / workbench.IMAGE).write_bytes(name.encode())
        image = root / "slots/b" / workbench.IMAGE
        value = workbench.checksum(image)
        evidence = root / "evidence"; self.results(evidence)
        state = dict(format=1, reference="a", experiment="b", reference_sha256=workbench.digest(root / "slots/a" / workbench.IMAGE))
        with patch.object(workbench, "ROOT", root): workbench.promote(state, image, evidence)
        recorded = json.loads((root / "state.json").read_text())
        self.assertEqual((recorded["reference"], recorded["experiment"], recorded["reference_sha256"]), ("b", "a", value))
        self.assertEqual((root / "slots/a" / workbench.IMAGE).read_bytes(), b"a")
        image.write_bytes(b"changed after acceptance")
        with self.assertRaisesRegex(ValueError, "checksum changed"):
            with patch.object(workbench, "ROOT", root): workbench.promote(state, image, evidence)

    def test_failed_regression_keeps_reference_and_cleans_build(self):
        root = self.root / "workbench"; root.mkdir()
        reference = self.marked(root / "slots/a") / workbench.IMAGE
        reference.write_bytes(b"accepted")
        workbench.checksum(reference)
        output = self.marked(root / "slots/b")
        (output / workbench.IMAGE).write_bytes(b"failed experiment")
        (self.root / "inputs.lock.json").write_text("original inputs")
        (output / (workbench.IMAGE + ".inputs.lock.json")).write_text("original inputs")
        (output / ".build").mkdir()
        (output / ".build/cache").write_bytes(b"regenerable")
        state = dict(format=1, reference="a", experiment="b", inputs=str(self.root), reference_sha256=workbench.digest(reference))
        args = argparse.Namespace(mode="test", replace=None)
        with (root / ".lock").open("a+") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with patch.object(workbench, "ROOT", root), patch.object(workbench, "regression", return_value=1):
                with self.assertRaisesRegex(ValueError, "reference retained"):
                    workbench.run_experiment(state, args, lock)
        self.assertEqual(reference.read_bytes(), b"accepted")
        self.assertEqual((output / workbench.IMAGE).read_bytes(), b"failed experiment")
        self.assertFalse((output / ".build").exists())
        self.assertEqual(state["reference"], "a")

    def test_retest_refuses_failed_build_from_another_input_snapshot(self):
        root = self.root / "managed"; root.mkdir()
        reference = self.marked(root / "slots/a") / workbench.IMAGE
        reference.write_bytes(b"accepted reference")
        inputs = self.root / "original-inputs"; inputs.mkdir()
        (inputs / "inputs.lock.json").write_text("old kernel snapshot")
        output = self.marked(root / "slots/b")
        candidate = output / workbench.IMAGE; candidate.write_bytes(b"failed new kernel build")
        (output / (workbench.IMAGE + ".inputs.lock.json")).write_text("new kernel snapshot")
        state = dict(format=1, reference="a", experiment="b", inputs=str(inputs), reference_sha256=workbench.digest(reference))
        with (root / ".lock").open("a+") as lock:
            with patch.object(workbench, "ROOT", root), patch.object(workbench, "regression") as regression:
                with self.assertRaisesRegex(ValueError, "input lock differs"):
                    workbench.run_experiment(state, argparse.Namespace(mode="test", replace=None), lock)
                regression.assert_not_called()
        self.assertEqual(reference.read_bytes(), b"accepted reference")
        self.assertEqual(candidate.read_bytes(), b"failed new kernel build")
        self.assertEqual(state["inputs"], str(inputs))

    def test_retest_can_select_only_the_exact_candidate_snapshot(self):
        root = self.root / "managed"; root.mkdir()
        reference = self.marked(root / "slots/a") / workbench.IMAGE
        reference.write_bytes(b"accepted reference")
        output = self.marked(root / "slots/b")
        candidate = output / workbench.IMAGE; candidate.write_bytes(b"new kernel experiment")
        (output / (workbench.IMAGE + ".inputs.lock.json")).write_text("new kernel snapshot")
        inputs = self.root / "new-inputs"; inputs.mkdir()
        selected = inputs / "inputs.lock.json"; selected.write_text("wrong snapshot")
        state = dict(format=1, reference="a", experiment="b", inputs="old-inputs", reference_sha256=workbench.digest(reference))
        args = argparse.Namespace(mode="test", replace=None, inputs=str(inputs))
        with (root / ".lock").open("a+") as lock:
            with patch.object(workbench, "ROOT", root), patch.object(subprocess, "run"), patch.object(workbench, "regression", return_value=0) as regression, patch.object(workbench, "promote") as promote:
                with self.assertRaisesRegex(ValueError, "input lock differs"):
                    workbench.run_experiment(state, args, lock)
                regression.assert_not_called(); promote.assert_not_called()
                selected.write_text("new kernel snapshot")
                workbench.run_experiment(state, args, lock)
                self.assertEqual(regression.call_args.args[0]["LUNA_CANONICAL_SKIP_BUILD"], "1")
                self.assertEqual(promote.call_args.args[0]["inputs"], str(inputs))
        self.assertEqual(reference.read_bytes(), b"accepted reference")
        self.assertEqual(candidate.read_bytes(), b"new kernel experiment")
        self.assertEqual(state["inputs"], "old-inputs")

    def test_new_input_snapshot_is_published_only_with_accepted_build(self):
        root = self.root / "managed"; root.mkdir()
        image = self.marked(root / "slots/a") / workbench.IMAGE
        image.write_bytes(b"initial"); workbench.checksum(image)
        inputs = self.root / "new-inputs"; inputs.mkdir()
        state = dict(format=1, reference="a", experiment="b", inputs="old-inputs", reference_sha256=workbench.digest(image))
        args = argparse.Namespace(mode="build", replace=None, inputs=str(inputs))
        original_run = subprocess.run
        def verified(command, **kwargs):
            if command[1].endswith("canonical-luna-inputs.py"):
                self.assertEqual(command[-1], str(inputs))
                return subprocess.CompletedProcess(command, 0)
            return original_run(command, **kwargs)
        def acceptance(environment, lock):
            self.assertEqual(environment["LUNA_CANONICAL_INPUT_ROOT"], str(inputs))
            candidate = Path(environment["LUNA_CANONICAL_IMAGE"])
            candidate.write_bytes(b"new inputs"); workbench.checksum(candidate)
            self.results(Path(environment["LUNA_CANONICAL_REGRESSION_ROOT"]))
            return 0
        with (root / ".lock").open("a+") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with patch.object(workbench, "ROOT", root), patch.object(subprocess, "run", side_effect=verified):
                with patch.object(workbench, "regression", return_value=1):
                    with self.assertRaisesRegex(ValueError, "reference retained"): workbench.run_experiment(state, args, lock)
                self.assertEqual(state["inputs"], "old-inputs")
                with patch.object(workbench, "regression", side_effect=acceptance): workbench.run_experiment(state, args, lock)
                self.assertEqual(workbench.load_state()["inputs"], str(inputs))
                with self.assertRaisesRegex(ValueError, "full build"):
                    workbench.run_experiment(state, argparse.Namespace(mode="resources", replace=[["resources/font", str(image)]], inputs=str(inputs)), lock)

    def test_input_snapshots_in_recycled_slots_are_rejected_before_cleanup(self):
        root = self.root / "managed"; root.mkdir()
        output = self.marked(root / "slots/b")
        inputs = output / "inputs"; inputs.mkdir()
        (inputs / "inputs.lock.json").write_text("keep snapshot")
        state = dict(format=1, reference="a", experiment="b", inputs=str(inputs), reference_sha256="unused")
        with (root / ".lock").open("a+") as lock:
            with patch.object(workbench, "ROOT", root):
                for option in (None, str(inputs)):
                    with self.assertRaisesRegex(ValueError, "outside the managed"):
                        workbench.run_experiment(state, argparse.Namespace(mode="build", replace=None, inputs=option), lock)
        self.assertEqual((inputs / "inputs.lock.json").read_text(), "keep snapshot")

    def test_successive_runs_reuse_two_slots_and_swap_roles(self):
        root = self.root / "managed"; root.mkdir()
        image = self.marked(root / "slots/a") / workbench.IMAGE
        image.write_bytes(b"initial reference"); workbench.checksum(image)
        state = dict(format=1, reference="a", experiment="b", inputs=str(self.root), reference_sha256=workbench.digest(image))
        counter = 0
        def accepted(environment, lock):
            nonlocal counter
            counter += 1
            candidate = Path(environment["LUNA_CANONICAL_IMAGE"])
            candidate.write_bytes(f"accepted {counter}".encode()); workbench.checksum(candidate)
            self.results(Path(environment["LUNA_CANONICAL_REGRESSION_ROOT"]))
            return 0
        with (root / ".lock").open("a+") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with patch.object(workbench, "ROOT", root), patch.object(workbench, "regression", side_effect=accepted):
                for role in ("b", "a", "b"):
                    workbench.run_experiment(state, argparse.Namespace(mode="build", replace=None), lock)
                    state = workbench.load_state()
                    self.assertEqual(state["reference"], role)
                    self.assertEqual({p.name for p in (root / "slots").iterdir()}, {"a", "b"})

    def test_killed_controller_leaves_lock_with_running_regression(self):
        scripts = self.root / "tools"; scripts.mkdir()
        (scripts / "test-canonical-luna-regression.sh").write_text('echo $$ > "$LUNA_TEST_PID"\nsleep 30\n')
        lock_path, pid_path = self.root / "lock", self.root / "child.pid"
        code = '''import fcntl, importlib.util, os, pathlib, sys
spec = importlib.util.spec_from_file_location("workbench", sys.argv[1])
w = importlib.util.module_from_spec(spec); spec.loader.exec_module(w)
w.REPO = pathlib.Path(sys.argv[2])
with open(sys.argv[3], "a+") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    env = os.environ.copy(); env["LUNA_TEST_PID"] = sys.argv[4]
    w.regression(env, lock)
'''
        controller = subprocess.Popen(["python3", "-c", code, str(REPO / "tools/canonical-luna-workbench.py"), str(self.root), str(lock_path), str(pid_path)])
        child = None
        try:
            deadline = time.monotonic() + 5
            while not pid_path.exists():
                if controller.poll() is not None or time.monotonic() > deadline: self.fail("regression child did not start")
                time.sleep(.02)
            child = int(pid_path.read_text())
            controller.kill(); controller.wait(timeout=5)
            with lock_path.open("a+") as other:
                with self.assertRaises(BlockingIOError): fcntl.flock(other, fcntl.LOCK_EX | fcntl.LOCK_NB)
        finally:
            if controller.poll() is None: controller.kill(); controller.wait(timeout=5)
            if child:
                try: os.killpg(child, signal.SIGKILL)
                except ProcessLookupError: pass


if __name__ == "__main__": unittest.main(verbosity=2)

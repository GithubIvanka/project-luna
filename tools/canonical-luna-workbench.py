#!/usr/bin/env python3
"""Two-slot canonical Luna development images, with separate durable evidence."""
import argparse
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parent.parent
ROOT = REPO / "target/canonical-workbench"
IMAGE = "luna-0.1.0.img"
HELPER = REPO / "tools/canonical-luna-artifacts.py"
spec = importlib.util.spec_from_file_location("canonical_luna_artifacts", HELPER)
artifacts = importlib.util.module_from_spec(spec)
spec.loader.exec_module(artifacts)


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def atomic_json(path, value):
    descriptor, name = tempfile.mkstemp(prefix=".state-", suffix=".tmp", dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(descriptor, "w") as out:
            json.dump(value, out, indent=2)
            out.write("\n")
            out.flush()
            os.fsync(out.fileno())
        temporary.replace(path)
        fd = os.open(path.parent, os.O_DIRECTORY)
        try: os.fsync(fd)
        finally: os.close(fd)
    finally:
        temporary.unlink(missing_ok=True)


def load_state():
    state = json.loads((ROOT / "state.json").read_text())
    if state.get("format") != 1 or {state.get("reference"), state.get("experiment")} != {"a", "b"}:
        raise ValueError("invalid workbench state")
    return state


def slot(state, role):
    return ROOT / "slots" / state[role]


def checksum(image):
    value = digest(image)
    image.with_suffix(image.suffix + ".sha256").write_text(f"{value}  {image}\n")
    return value


def copy_image(source, output, lock):
    source = artifacts.regular(source)
    subprocess.run(["cp", "--reflink=auto", "--sparse=always", str(source), str(output)], check=True, pass_fds=(lock.fileno(),))
    subprocess.run(["sync", "-f", str(output)], check=True)
    for suffix in (".BUILD-INFO", ".inputs.lock.json"):
        metadata = Path(str(source) + suffix)
        if metadata.is_file(): shutil.copyfile(metadata, Path(str(output) + suffix))
    return checksum(output)


def prepare_experiment(state, copy_reference, lock):
    directory = slot(state, "experiment")
    if directory.is_symlink(): raise ValueError("experiment slot is a symlink")
    if directory.exists():
        marker = directory / ".luna-canonical-output-v1"
        if marker.is_symlink() or not marker.is_file() or marker.read_text() != "Project Luna generated canonical output v1\n":
            raise ValueError("refusing to recycle an unmarked experiment slot")
        artifacts.cleanup_build(directory)
        shutil.rmtree(directory)
    directory.mkdir()
    (directory / ".luna-canonical-output-v1").write_text("Project Luna generated canonical output v1\n")
    if copy_reference:
        reference = slot(state, "reference") / IMAGE
        if digest(reference) != state["reference_sha256"]:
            raise ValueError("reference image changed outside workbench")
        copy_image(reference, directory / IMAGE, lock)
    return directory


def acceptance_complete(results, *, workbench_checks=False):
    expected = {"inputs", "input-negative-checks", "state-runtime", "init", "build", "structure", "production", "display", "login-1", "login-2", "login-3", "logout", "reboot", "shutdown"}
    if workbench_checks: expected.update({"artifact-checks", "app-runtime", "bundle-launch"})
    lines = results.read_text().splitlines()
    return bool(lines) and lines[-1] == "Canonical Luna regression: PASS" and all(f"{name} PASS" in lines for name in expected) and not any(line.endswith(" FAIL") for line in lines)


def promote(state, image, evidence):
    if not acceptance_complete(evidence / "results.txt", workbench_checks=True):
        raise ValueError("experiment did not complete canonical acceptance")
    current = digest(image)
    # Regression checks that QEMU did not mutate its source. Bind promotion to
    # the exact bytes it accepted, not a stale PASS file.
    expected = image.with_suffix(image.suffix + ".sha256").read_text().split()[0]
    if current != expected: raise ValueError("accepted image checksum changed")
    for path in (image, image.with_suffix(image.suffix + ".sha256"), evidence / "results.txt"):
        with artifacts.regular(path).open("rb") as durable:
            os.fsync(durable.fileno())
    for directory in (image.parent, image.parent.parent, evidence, evidence.parent):
        descriptor = os.open(directory, os.O_DIRECTORY)
        try: os.fsync(descriptor)
        finally: os.close(descriptor)
    state["reference"], state["experiment"] = state["experiment"], state["reference"]
    state["reference_sha256"] = current
    state["reference_evidence"] = str(evidence)
    state["updated"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
    atomic_json(ROOT / "state.json", state)


def regression(environment, lock):
    # Bash retains this open lock even if the Python controller is killed.
    # A new run cannot recycle the slot while the old regression still runs.
    process = subprocess.Popen(["bash", str(REPO / "tools/test-canonical-luna-regression.sh")], env=environment, start_new_session=True, pass_fds=(lock.fileno(),))
    try:
        return process.wait()
    finally:
        # Stop the entire child tree before cleanup or releasing the slot lock.
        try: os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError: pass
        try: process.wait(timeout=8)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
        try: os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError: pass


def prune_test_disks():
    evidence = ROOT / "evidence"
    if not evidence.exists(): return
    for run in evidence.iterdir():
        if not re.fullmatch(r"\d{8}-\d{6}-\d{6}", run.name) or run.is_symlink() or not run.is_dir(): continue
        marker = run / ".luna-canonical-evidence-v1"
        if marker.is_symlink() or not marker.is_file() or marker.read_text() != "Project Luna generated canonical evidence v1\n": continue
        names = [(f"login-{n}", "canonical-login-test.qcow2") for n in (1, 2, 3)] + [(action, "guest.qcow2") for action in ("logout", "reboot", "shutdown", "bundle-launch")] + [("alpha-design", "alpha.qcow2")]
        for case, name in names:
            disk = run / case / name
            if disk.exists() or disk.is_symlink():
                artifacts.regular(disk).unlink()


def run_experiment(state, args, lock):
    if args.mode == "resources" and not args.replace:
        raise ValueError("resources requires --replace LOGICAL SOURCE")
    inputs = state["inputs"]
    if getattr(args, "inputs", None):
        if args.mode not in ("build", "test"): raise ValueError("a new input snapshot requires a full build")
        inputs = str(Path(args.inputs).resolve())
    if Path(inputs).resolve().is_relative_to(ROOT):
        raise ValueError("immutable inputs must remain outside the managed workbench")
    # Desktop-набор требует независимого Wayland-бандла. Проверяем условие
    # до переиспользования слота и длительной сборки, а не в конце QEMU.
    if (Path(inputs) / "desktop-bundles").is_dir() and not os.environ.get("LUNA_CANONICAL_WAYLAND_FIXTURE"):
        raise ValueError("Для проверки Alpha задайте LUNA_CANONICAL_WAYLAND_FIXTURE: проверенный Weston-бандл для выбранного входного снимка")
    if getattr(args, "inputs", None):
        subprocess.run(["python3", str(REPO / "tools/canonical-luna-inputs.py"), "verify", inputs], check=True)
    reference = slot(state, "reference") / IMAGE
    original_hash = state["reference_sha256"]
    if digest(reference) != original_hash: raise ValueError("reference checksum mismatch")
    prune_test_disks()
    if args.mode == "test":
        output = slot(state, "experiment")
        if not (output / IMAGE).is_file(): raise ValueError("no existing experiment to retest")
        candidate_lock = artifacts.regular(output / (IMAGE + ".inputs.lock.json"))
        selected_lock = artifacts.regular(Path(inputs) / "inputs.lock.json")
        if digest(candidate_lock) != digest(selected_lock):
            raise ValueError("experiment input lock differs from the selected snapshot; select its exact snapshot with run test --inputs or use run build --inputs")
    else:
        output = prepare_experiment(state, args.mode == "resources", lock)
    evidence = ROOT / "evidence" / (time.strftime("%Y%m%d-%H%M%S") + f"-{time.time_ns() % 1000000:06d}")
    environment = os.environ.copy()
    environment.update(LUNA_CANONICAL_REGRESSION_ROOT=str(evidence), LUNA_CANONICAL_OUTPUT_ROOT=str(output),
        LUNA_CANONICAL_IMAGE=str(output / IMAGE), LUNA_CANONICAL_INPUT_ROOT=inputs,
        LUNA_CANONICAL_KEEP_TEST_DISKS="0", LUNA_CANONICAL_MANAGED_RUN="1")
    environment["LUNA_CANONICAL_SKIP_BUILD"] = "0" if args.mode == "build" else "1"
    evidence.mkdir(parents=True)
    # The regression owns the evidence directory; reserve only its name here.
    evidence.rmdir()
    try:
        if args.mode == "resources":
            result = artifacts.patch_system_image(output / IMAGE, args.replace, workbench_lock=lock)
            checksum(output / IMAGE)
            evidence.mkdir(parents=True)
            (evidence / "resource-patch.json").write_text(json.dumps(result, indent=2) + "\n")
            # Regression allows an evidence directory created by the workbench
            # only before results/logs exist.
        if regression(environment, lock) != 0:
            raise ValueError(f"acceptance failed; reference retained; evidence: {evidence}")
        promote(dict(state, inputs=inputs), output / IMAGE, evidence)
        print(f"Reference promoted: {output / IMAGE}\nEvidence: {evidence}")
    finally:
        if output.exists():
            subprocess.run(["python3", str(HELPER), "cleanup-build", str(output)], check=True)
        # Preserve a failed experiment and all its logs, never promote it.
        if digest(reference) != original_hash:
            raise ValueError("reference unexpectedly changed")


def view_reference(state, args, lock):
    image = artifacts.regular(slot(state, "reference") / IMAGE)
    original_hash = digest(image)
    if original_hash != state["reference_sha256"]:
        raise ValueError("reference image changed outside workbench")
    evidence = ROOT / "evidence" / (time.strftime("%Y%m%d-%H%M%S") + f"-{os.getpid():06d}")
    evidence.mkdir(parents=True)
    (evidence / ".luna-canonical-evidence-v1").write_text("Project Luna generated canonical evidence v1\n")
    atomic_json(evidence / "view.json", {"reference": state["reference"], "sha256": original_hash})
    print(f"Графический QEMU: {evidence}", flush=True)
    process = None
    try:
        # Третий постоянный диск не создаётся. QEMU сам удаляет snapshot-слой,
        # а OVMF и управляющие сокеты удаляются при выходе из этого контекста.
        with tempfile.TemporaryDirectory(prefix="luna-view-") as temporary:
            variables = Path(temporary) / "vars.fd"
            shutil.copyfile("/usr/share/OVMF/OVMF_VARS_4M.fd", variables)
            command = ["qemu-system-x86_64", "-name", "Luna Alpha", "-machine", "q35",
                "-m", "2G", "-smp", "4", "-accel", "kvm", "-cpu", "host",
                "-vga", "none", "-device", "virtio-vga-gl", "-nic", "user,model=virtio-net-pci",
                "-device", "qemu-xhci,id=pointer-usb", "-device", "usb-tablet,bus=pointer-usb.0",
                "-drive", "if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd",
                "-drive", f"if=pflash,format=raw,file={variables}",
                "-drive", f"format=raw,snapshot=on,file={image}",
                "-serial", f"file:{evidence}/serial.log",
                "-qmp", f"unix:{temporary}/qmp.sock,server=on,wait=off",
                "-monitor", f"unix:{temporary}/monitor.sock,server=on,wait=off",
                "-display", "gtk,gl=on,zoom-to-fit=on,show-tabs=off"]
            if not args.windowed:
                command.append("-full-screen")
            atomic_json(evidence / "command.json", command)
            with (evidence / "qemu.log").open("wb") as output:
                process = subprocess.Popen(command, stdout=output, stderr=output,
                    start_new_session=True, pass_fds=(lock.fileno(),))
                atomic_json(evidence / "control.json", {"pid": process.pid, "qmp": str(Path(temporary) / "qmp.sock")})
                try:
                    result = process.wait()
                finally:
                    if process.poll() is None:
                        process.terminate()
                        try: process.wait(timeout=5)
                        except subprocess.TimeoutExpired: process.kill(); process.wait()
            (evidence / "results.txt").write_text(f"QEMU exit code: {result}\n")
            if result:
                raise RuntimeError(f"graphical QEMU failed; see {evidence}/qemu.log")
    finally:
        if digest(image) != original_hash:
            raise ValueError("reference unexpectedly changed")


def main():
    def cancel(signum, frame):
        raise KeyboardInterrupt(f"workbench interrupted by signal {signum}")
    signal.signal(signal.SIGTERM, cancel)
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    p = commands.add_parser("init"); p.add_argument("--image", required=True); p.add_argument("--results", required=True); p.add_argument("--inputs", required=True)
    commands.add_parser("status")
    p = commands.add_parser("view", help="открыть принятый образ в графическом QEMU без сохранения изменений гостя")
    p.add_argument("--windowed", action="store_true", help="открыть обычное окно вместо полноэкранного режима")
    p = commands.add_parser("run"); p.add_argument("mode", choices=("build", "resources", "test")); p.add_argument("--replace", action="append", nargs=2)
    p.add_argument("--inputs", help="immutable snapshot for build, or the exact existing experiment snapshot for test; accepted only on PASS")
    args = parser.parse_args()
    for component in [ROOT, *ROOT.parents]:
        if component == REPO: break
        if component.is_symlink(): raise ValueError("symlink in managed workbench root")
    if not ROOT.exists(): ROOT.mkdir(mode=0o700)
    marker = ROOT / ".luna-workbench-v1"
    if not marker.exists():
        if any(ROOT.iterdir()): raise ValueError("refusing to adopt a nonempty unmarked workbench")
        marker.write_text("Project Luna generated development artifacts v1\n")
    if marker.is_symlink() or marker.read_text() != "Project Luna generated development artifacts v1\n": raise ValueError("invalid workbench marker")
    if (ROOT / ".lock").is_symlink(): raise ValueError("workbench lock is a symlink")
    with (ROOT / ".lock").open("a+") as lock:
        # Просмотр и чтение состояния совместимы; сборка/принятие ждут,
        # пока все графические читатели освободят неизменяемый слот.
        mode = fcntl.LOCK_SH if args.command in ("status", "view") else fcntl.LOCK_EX
        fcntl.flock(lock, mode | fcntl.LOCK_NB)
        if (ROOT / "slots").is_symlink(): raise ValueError("slots directory is a symlink")
        (ROOT / "slots").mkdir(exist_ok=True)
        for path in [ROOT / "slots/a", ROOT / "slots/b", ROOT / "state.json", ROOT / "evidence"]:
            if path.is_symlink(): raise ValueError("symlink in managed artifacts")
        if args.command == "init":
            if (ROOT / "state.json").exists(): raise ValueError("workbench already initialized")
            if not acceptance_complete(Path(args.results)): raise ValueError("reference results are not a complete accepted regression")
            inputs = Path(args.inputs).resolve()
            if inputs.is_relative_to(ROOT): raise ValueError("immutable inputs must remain outside the managed workbench")
            if not (inputs / "inputs.lock.json").is_file(): raise ValueError("missing immutable input snapshot")
            source = artifacts.regular(args.image)
            expected = source.with_suffix(source.suffix + ".sha256").read_text().split()[0]
            if digest(source) != expected: raise ValueError("reference source checksum mismatch")
            reference = ROOT / "slots/a"
            if reference.exists():
                owned = reference / ".luna-canonical-output-v1"
                allowed = {owned.name, IMAGE, IMAGE + ".sha256", IMAGE + ".BUILD-INFO", IMAGE + ".inputs.lock.json"}
                if owned.is_symlink() or not owned.is_file() or owned.read_text() != "Project Luna generated canonical output v1\n" or any(p.name not in allowed or p.is_symlink() or not p.is_file() for p in reference.iterdir()):
                    raise ValueError("refusing to replace unknown interrupted import contents")
                shutil.rmtree(reference)
            reference.mkdir()
            (reference / ".luna-canonical-output-v1").write_text("Project Luna generated canonical output v1\n")
            try:
                value = copy_image(source, reference / IMAGE, lock)
                for path in reference.iterdir():
                    with artifacts.regular(path).open("rb") as durable: os.fsync(durable.fileno())
                for directory in (reference, reference.parent):
                    descriptor = os.open(directory, os.O_DIRECTORY)
                    try: os.fsync(descriptor)
                    finally: os.close(descriptor)
                state = dict(format=1, reference="a", experiment="b", reference_sha256=value,
                    reference_evidence=str(Path(args.results).absolute().parent), inputs=str(inputs))
                atomic_json(ROOT / "state.json", state)
            except BaseException:
                if not (ROOT / "state.json").exists(): shutil.rmtree(reference)
                raise
            print(json.dumps(state, indent=2))
        elif args.command == "status": print(json.dumps(load_state(), indent=2))
        elif args.command == "view": view_reference(load_state(), args, lock)
        else: run_experiment(load_state(), args, lock)


if __name__ == "__main__": main()

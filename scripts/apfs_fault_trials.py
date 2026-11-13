"""Explicit generated-image fault trials; no physical/native qualification.

Authored source, not executed here. Darwin only at invocation. Fixed hdiutil
argv controls only the image created in this utility's fresh private workspace.
"""
from __future__ import annotations
import argparse
from contextlib import contextmanager
try:
    import fcntl
except ImportError:
    fcntl = None
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import uuid

HDIUTIL = "/usr/bin/hdiutil"
MAX_META = 1_048_576
PROBE_BYTES = 1_048_576
LOCK = b"optiflow.apfs-fault-lock.v1\n"
SCHEMA = "optiflow.apfs-fault-observation.v1"
NAMES = {"creation-intent.json", "creation.json", "trial.lock", "trial.dmg", "mount", "trial.json"}
for _phase in ["disconnect", "reconnect", "finish"]:
    NAMES.update({_phase + "-intent.json", _phase + ".json"})


class Refusal(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Refusal(message)


def closed(value, names):
    require(type(value) is dict and set(value) == set(names), "closed trial metadata required")
    return value


def sha(data):
    return hashlib.sha256(data).hexdigest()


def pin(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def unique(pairs):
    value = {}
    for key, item in pairs:
        require(key not in value, "duplicate trial metadata member")
        value[key] = item
    return value


def signature(info):
    return (info.st_dev, info.st_ino, info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns, info.st_mode)


def regular(path, maximum):
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and 0 < before.st_size <= maximum,
            "bounded singly linked regular trial member required")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        require(signature(os.fstat(descriptor)) == signature(before), "trial member changed while opening")
        data = bytearray()
        while len(data) <= maximum:
            chunk = os.read(descriptor, min(65_536, maximum + 1 - len(data)))
            if not chunk:
                break
            data.extend(chunk)
        require(len(data) == before.st_size and signature(os.fstat(descriptor)) == signature(before)
                and signature(path.lstat()) == signature(before), "trial member changed while reading")
        return bytes(data)
    finally:
        os.close(descriptor)


def document(path):
    return json.loads(regular(path, MAX_META), object_pairs_hook=unique,
                      parse_constant=lambda _: require(False, "finite trial JSON required"))


def new_file(path, data):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    with os.fdopen(descriptor, "wb") as output:
        output.write(data)
        output.flush()
        os.fsync(output.fileno())


def new_json(path, value):
    data = (json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n").encode()
    require(0 < len(data) <= MAX_META, "trial record byte bound exceeded")
    new_file(path, data)
    require(regular(path, MAX_META) == data, "created trial record changed")
    return sha(data)


def private_root(root):
    require(root.is_absolute() and root.resolve(strict=True) == root, "canonical absolute trial workspace required")
    info = root.lstat()
    require(stat.S_ISDIR(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o700 and info.st_uid == os.geteuid(),
            "owned private trial workspace required")
    require({p.name for p in root.iterdir()} <= NAMES, "unknown workspace material; preserve it")
    return info


@contextmanager
def locked(root, create=False):
    require(fcntl is not None, "native trial locking is unavailable")
    flags = os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
    descriptor = os.open(root / "trial.lock", flags | (os.O_CREAT | os.O_EXCL if create else 0), 0o600)
    try:
        info = os.fstat(descriptor)
        require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_uid == os.geteuid(), "owned trial lock required")
        fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if create:
            os.write(descriptor, LOCK)
            os.fsync(descriptor)
        require(os.pread(descriptor, 128, 0) == LOCK
                and signature(os.fstat(descriptor)) == signature((root / "trial.lock").lstat()), "trial lock marker/identity differs")
        yield
    finally:
        os.close(descriptor)


def native(root, operation, args):
    fixed = {
        "info": ["info", "-plist"],
        "attach": ["attach", "-nobrowse", "-plist", "-mountpoint", str(root / "mount"), str(root / "trial.dmg")],
        "detach": ["detach", str(root / "mount")],
        "detach_force": ["detach", "-force", str(root / "mount")],
    }
    if operation == "create":
        require(len(args) == 10 and args[:2] == ["create", "-size"] and args[2] in ["256m", "512m"]
                and args[3:8] == ["-fs", "APFS", "-format", "UDRW", "-volname"]
                and re.fullmatch(r"OptiflowTrial-[0-9a-f]{8}", args[8]) and args[9] == str(root / "trial.dmg"),
                "only bounded generated image creation is permitted")
    else:
        require(operation in fixed and args == fixed[operation], "only fixed owned-image native argv is permitted")
    private_root(root)
    # Fixed operating-system utility only. Spooling bounds memory; metadata
    # parsing is capped. A timeout leaves native/helper state uncertain.
    with tempfile.TemporaryFile(dir=root) as stdout, tempfile.TemporaryFile(dir=root) as stderr:
        result = subprocess.run([HDIUTIL, *args], cwd=root, stdin=subprocess.DEVNULL,
                                stdout=stdout, stderr=stderr, timeout=30, check=False,
                                env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "LC_ALL": "C"})
        require(stdout.tell() <= MAX_META and stderr.tell() <= MAX_META, "native metadata capture exceeds1MiB")
        stdout.seek(0); stderr.seek(0)
        out, err = stdout.read(MAX_META + 1), stderr.read(MAX_META + 1)
    capture = {"operation": operation, "exit_code": result.returncode,
               "stdout_sha256": sha(out), "stdout_bytes": len(out), "stderr_sha256": sha(err), "stderr_bytes": len(err)}
    require(result.returncode == 0, "hdiutil operation failed; retain workspace and inspect uncertain native state")
    return out, capture


def matching_image(info, root):
    require(type(info) is dict and isinstance(info.get("images"), list) and len(info["images"]) <= 128,
            "bounded native image inventory required")
    matches = [image for image in info["images"] if type(image) is dict and image.get("image-path") == str(root / "trial.dmg")]
    require(len(matches) <= 1, "ambiguous owned image association")
    if not matches:
        return None
    entities = matches[0].get("system-entities")
    require(isinstance(entities, list) and 1 <= len(entities) <= 32, "bounded owned image entities required")
    mounts = [entry for entry in entities if type(entry) is dict and entry.get("mount-point")]
    require(len(mounts) == 1 and mounts[0]["mount-point"] == str(root / "mount"),
            "owned image has a foreign or ambiguous mount; refuse detachment")
    device = mounts[0].get("dev-entry")
    require(isinstance(device, str) and re.fullmatch(r"/dev/disk[0-9]+(?:s[0-9]+)*", device), "native image device identity unavailable")
    return {"device": device, "mount": str(root / "mount")}


def observe_image(root):
    data, capture = native(root, "info", ["info", "-plist"])
    return matching_image(plistlib.loads(data), root), capture


def host_target():
    require(sys.platform == "darwin", "APFS fault utility requires native macOS; no simulation was run")
    target = {"arm64": "aarch64-apple-darwin", "x86_64": "x86_64-apple-darwin"}.get(platform.machine())
    require(target is not None, "supported native Mac architecture required")
    return target


def load_trial(root, expected):
    info = private_root(root)
    data = regular(root / "trial.json", MAX_META)
    require(pin(expected) and sha(data) == expected, "exact externally retained trial digest required")
    trial = json.loads(data, object_pairs_hook=unique)
    closed(trial, ["schema", "root", "root_device", "root_inode", "image_device", "image_inode", "maximum_image_bytes",
                   "host_target", "probe", "fixture_policy", "physical_disconnect_observed", "native_qualification_established"])
    require(trial["schema"] == "optiflow.apfs-fault-trial.v1" and trial["root"] == str(root)
            and type(trial["root_device"]) is int and type(trial["root_inode"]) is int
            and (trial["root_device"], trial["root_inode"]) == (info.st_dev, info.st_ino)
            and trial["host_target"] == host_target() and trial["fixture_policy"] == "generated_only"
            and trial["physical_disconnect_observed"] is False and trial["native_qualification_established"] is False,
            "trial identity or evidence scope differs")
    image = (root / "trial.dmg").lstat()
    require(stat.S_ISREG(image.st_mode) and image.st_nlink == 1
            and type(trial["maximum_image_bytes"]) is int and 0 < image.st_size <= trial["maximum_image_bytes"] <= 553_648_128
            and type(trial["image_device"]) is int and type(trial["image_inode"]) is int
            and (image.st_dev, image.st_ino) == (trial["image_device"], trial["image_inode"]), "created image identity/bound differs")
    closed(trial["probe"], ["device", "inode", "byte_count", "sha256"])
    require(all(type(trial["probe"][key]) is int for key in ["device", "inode", "byte_count"])
            and trial["probe"]["byte_count"] == PROBE_BYTES and pin(trial["probe"]["sha256"]), "generated probe identity differs")
    return trial


def reserve(root, phase, trial_sha256):
    require(phase in ["disconnect", "reconnect", "finish"], "recognized trial phase required")
    require(not os.path.lexists(root / (phase + ".json")), "phase result already exists; never replay")
    new_json(root / (phase + "-intent.json"), {"schema": "optiflow.apfs-fault-intent.v1", "phase": phase,
                                             "trial_sha256": trial_sha256, "replay_authority": False})


def observation(trial_sha256, phase, attached, captures, read=None):
    return {"schema": SCHEMA, "phase": phase, "trial_sha256": trial_sha256,
            "evidence_kind": "simulated_apfs_image_fault", "image_attached_observed": attached,
            "physical_disconnect_observed": False, "native_qualification_established": False,
            "commands": captures, "descriptor_read": read}


def previous_phase(root, phase, expected, trial_sha256):
    data = regular(root / (phase + ".json"), MAX_META)
    require(pin(expected) and sha(data) == expected, "exact preceding fault-record digest required")
    value = json.loads(data, object_pairs_hook=unique)
    closed(value, observation(trial_sha256, phase, False, []))
    require(value["schema"] == SCHEMA and value["phase"] == phase and value["trial_sha256"] == trial_sha256
            and value["evidence_kind"] == "simulated_apfs_image_fault"
            and value["physical_disconnect_observed"] is False and value["native_qualification_established"] is False
            and value["image_attached_observed"] is (phase == "reconnect"), "preceding fault phase/scope differs")
    return value


def create(root, size_mib):
    target = host_target()
    require(root.is_absolute() and root.parent.resolve(strict=True) == root.parent and root.name not in ["", ".", ".."],
            "fresh canonical absolute workspace required")
    require(size_mib in [256, 512] and shutil.disk_usage(root.parent).free >= (size_mib + 64) * 1_048_576,
            "bounded image size plus observed reserve required; this is not a reservation")
    root.mkdir(mode=0o700)
    with locked(root, create=True):
        new_json(root / "creation-intent.json", {"schema": "optiflow.apfs-fault-creation.v1", "size_mib": size_mib, "generated_only": True})
        (root / "mount").mkdir(mode=0o700)
        _, created = native(root, "create", ["create", "-size", str(size_mib) + "m", "-fs", "APFS", "-format", "UDRW",
                                            "-volname", "OptiflowTrial-" + uuid.uuid4().hex[:8], str(root / "trial.dmg")])
        _, attached = native(root, "attach", ["attach", "-nobrowse", "-plist", "-mountpoint", str(root / "mount"), str(root / "trial.dmg")])
        image, observed = observe_image(root)
        require(image is not None, "new image attachment was not independently observed")
        collection = root / "mount" / "collection"; collection.mkdir(mode=0o700)
        data = bytes(range(256)) * 4096
        for name, content in [("keeper.bin", data), ("candidate.bin", data), ("different.bin", b"x" * PROBE_BYTES)]:
            new_file(collection / name, content)
        probe = root / "mount" / "io-probe.bin"; new_file(probe, data)
        metadata, image_stat, probe_stat = private_root(root), (root / "trial.dmg").lstat(), probe.lstat()
        trial = {"schema": "optiflow.apfs-fault-trial.v1", "root": str(root), "root_device": metadata.st_dev,
                 "root_inode": metadata.st_ino, "image_device": image_stat.st_dev, "image_inode": image_stat.st_ino,
                 "maximum_image_bytes": (size_mib + 16) * 1_048_576, "host_target": target,
                 "probe": {"device": probe_stat.st_dev, "inode": probe_stat.st_ino, "byte_count": len(data), "sha256": sha(data)},
                 "fixture_policy": "generated_only", "physical_disconnect_observed": False, "native_qualification_established": False}
        digest = new_json(root / "trial.json", trial)
        new_json(root / "creation.json", observation(digest, "create", True, [created, attached, observed]))
        return "trial.json", digest


def transition(root, phase, trial_sha256, previous_sha256=None):
    require(phase in ["disconnect", "reconnect", "finish"], "explicit recognized trial transition required")
    host_target()
    private_root(root)
    with locked(root):
        trial = load_trial(root, trial_sha256)
        if phase in ["reconnect", "finish"]:
            previous_phase(root, "disconnect" if phase == "reconnect" else "reconnect", previous_sha256, trial_sha256)
        before, capture = observe_image(root); captures = [capture]
        require((before is None) == (phase == "reconnect"), "current image attachment does not match selected phase")
        reserve(root, phase, trial_sha256)
        read = None
        if phase == "disconnect":
            descriptor = os.open(root / "mount" / "io-probe.bin", os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
            try:
                probe = os.fstat(descriptor); expected = trial["probe"]
                require(stat.S_ISREG(probe.st_mode) and probe.st_nlink == 1
                        and (probe.st_dev, probe.st_ino, probe.st_size) == (expected["device"], expected["inode"], expected["byte_count"]),
                        "generated held probe changed")
                _, capture = native(root, "detach_force", ["detach", "-force", str(root / "mount")]); captures.append(capture)
                try:
                    data = os.pread(descriptor, 65_536, PROBE_BYTES - 65_536)
                    read = {"state": "read_returned", "byte_count": len(data), "errno": None}
                except OSError as failure:
                    read = {"state": "os_error", "byte_count": None, "errno": failure.errno}
            finally:
                os.close(descriptor)
        elif phase == "reconnect":
            _, capture = native(root, "attach", ["attach", "-nobrowse", "-plist", "-mountpoint", str(root / "mount"), str(root / "trial.dmg")]); captures.append(capture)
        else:
            _, capture = native(root, "detach", ["detach", str(root / "mount")]); captures.append(capture)
        after, capture = observe_image(root); captures.append(capture)
        require((after is not None) == (phase == "reconnect"), "fault transition attachment is uncertain; retain intent, never replay")
        result = observation(trial_sha256, phase, after is not None, captures, read)
        return phase + ".json", new_json(root / (phase + ".json"), result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="phase", required=True)
    for phase in ["create", "disconnect", "reconnect", "finish"]:
        action = actions.add_parser(phase)
        action.add_argument("--workspace", required=True, type=Path)
        action.add_argument("--acknowledge-generated-volume-simulation", action="store_true", required=True)
        if phase == "create": action.add_argument("--size-mib", type=int, choices=[256, 512], default=256)
        else: action.add_argument("--trial-sha256", required=True)
        if phase == "reconnect": action.add_argument("--disconnect-sha256", required=True)
        if phase == "finish": action.add_argument("--reconnect-sha256", required=True)
    args = parser.parse_args()
    try:
        if args.phase == "create": name, digest = create(args.workspace, args.size_mib)
        else: name, digest = transition(args.workspace, args.phase, args.trial_sha256, getattr(args, "disconnect_sha256", getattr(args, "reconnect_sha256", None)))
        print(json.dumps({"schema": "optiflow.apfs-fault-command.v1", "phase": args.phase, "record_file": name,
                          "record_sha256": digest, "physical_disconnect_observed": False, "native_qualification_established": False}))
        return 0
    except (OSError, ValueError, subprocess.TimeoutExpired) as failure:
        print(json.dumps({"schema": "optiflow.apfs-fault-command.v1", "phase": args.phase, "state": "refused_or_uncertain",
                          "reason": str(failure) if isinstance(failure, Refusal) else "native/OS operation failed or timed out; no completion is inferred",
                          "next_action": "preserve the generated workspace; inspect fixed trial/intent records and actual image state before any manual recovery",
                          "physical_disconnect_observed": False, "native_qualification_established": False}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

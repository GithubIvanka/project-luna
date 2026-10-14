#!/usr/bin/env python3
"""Check generated Btrfs inode metadata without mounting or changing its image."""
import re
import subprocess
import sys


def verify(image):
    dump = subprocess.run(["btrfs", "inspect-internal", "dump-tree", "-t", "fs", image],
                          check=True, capture_output=True, text=True).stdout
    inodes, children = {}, {}
    item = None
    target = None
    for line in dump.splitlines():
        match = re.search(r"item \d+ key \((\d+) (INODE_ITEM|DIR_ITEM) \d+\)", line)
        if match:
            item = (int(match[1]), match[2])
            target = None
        if item and item[1] == "INODE_ITEM":
            match = re.search(r"mode ([0-7]+) links \d+ uid (\d+) gid (\d+)", line)
            if match:
                inodes[item[0]] = tuple(int(match[i], 8 if i == 1 else 10) for i in (1, 2, 3))
        if item and item[1] == "DIR_ITEM":
            match = re.search(r"location key \((\d+) INODE_ITEM 0\)", line)
            if match:
                target = int(match[1])
            match = re.search(r"name: (.+)$", line)
            if match and target is not None:
                children[(item[0], match[1])] = target
                target = None
    expected = {path: (0, 0) for path in ("", "system", "system/apps", "system/state", "system/config", "users")}
    expected.update({path: (1000, 100) for path in ("users/luna", "users/luna/home", "users/luna/data", "users/luna/config")})
    for path, owner in expected.items():
        inode = 256
        for part in path.split("/") if path else []:
            inode = children[(inode, part)]
        mode, uid, gid = inodes[inode]
        if mode & 0o170000 != 0o040000 or (uid, gid) != owner or mode & 0o022:
            raise ValueError(f"unsafe generated DATA /{path}: mode={mode:o}, uid={uid}, gid={gid}; expected owner={owner}, no group/other writes")
    print("Canonical DATA inode ownership: PASS")


if __name__ == "__main__":
    verify(sys.argv[1])

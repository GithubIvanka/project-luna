#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:?rootfs path required}"

# Disposable QEMU checkpoint credentials.
# This image is only a local interactive test fixture.
mkdir -p "$ROOT/home/luna"
chown 1000:1000 "$ROOT/home/luna"
chmod 700 "$ROOT/home/luna"

# Create the normal non-root Luna test account.
sed -i '/^luna:/d' "$ROOT/etc/passwd"
printf '%s\n' 'luna:x:1000:100:Luna User:/home/luna:/bin/ash' >> "$ROOT/etc/passwd"

# Generate real shadow hashes with Alpine's BusyBox mkpasswd. The test
# credentials are luna/luna and root/root.
LUNA_HASH="$("$ROOT/bin/busybox" mkpasswd luna)"
ROOT_HASH="$("$ROOT/bin/busybox" mkpasswd root)"
sed -i "s|^root:[^:]*:|root:${ROOT_HASH}:|" "$ROOT/etc/shadow"
sed -i '/^luna:/d' "$ROOT/etc/shadow"
PASS_DATE=$(( $(date +%s) / 86400 ))
printf '%s\n' "luna:${LUNA_HASH}:${PASS_DATE}:0:99999:7:::" >> "$ROOT/etc/shadow"

# Keep the account in the groups useful for the upcoming graphical tests.
sed -i 's/^wheel:x:10:root$/wheel:x:10:root,luna/' "$ROOT/etc/group"
sed -i 's/^audio:x:18:$/audio:x:18:luna/' "$ROOT/etc/group"
sed -i 's/^input:x:23:$/input:x:23:luna/' "$ROOT/etc/group"
sed -i 's/^video:x:27:root$/video:x:27:root,luna/' "$ROOT/etc/group"
sed -i 's/^netdev:x:28:$/netdev:x:28:luna/' "$ROOT/etc/group"

if [[ "${LUNA_ALPINE_TEST_MULTIUSER:-0}" == "1" ]]; then
    for spec in "alice:1001:Alice" "bob:1002:Bob"; do
        IFS=: read -r USERNAME USER_ID DISPLAY_NAME <<< "$spec"
        mkdir -p "$ROOT/home/$USERNAME"
        if [[ "$(id -u)" == "0" ]]; then
            chown "$USER_ID:100" "$ROOT/home/$USERNAME"
            chmod 700 "$ROOT/home/$USERNAME"
        else
            chmod 755 "$ROOT/home/$USERNAME"
        fi

        sed -i "/^${USERNAME}:/d" "$ROOT/etc/passwd"
        printf '%s\n' "${USERNAME}:x:${USER_ID}:100:${DISPLAY_NAME}:/home/${USERNAME}:/bin/ash" >> "$ROOT/etc/passwd"

        HASH="$("$ROOT/bin/busybox" mkpasswd luna)"
        sed -i "/^${USERNAME}:/d" "$ROOT/etc/shadow"
        printf '%s\n' "${USERNAME}:${HASH}:${PASS_DATE}:0:99999:7:::" >> "$ROOT/etc/shadow"

        for GROUP in wheel audio input video netdev; do
            sed -i -E "s/^(${GROUP}:x:[^:]*:.*)$/\1,${USERNAME}/" "$ROOT/etc/group"
        done
    done
fi

chmod 640 "$ROOT/etc/shadow"

# Validate the two interactive accounts before the image is created.
awk -F: '($1=="root" || $1=="luna" || $1=="alice" || $1=="bob") && NF != 9 { bad=1; print "invalid shadow record: "$0 > "/dev/stderr" } END { exit bad }' "$ROOT/etc/shadow"

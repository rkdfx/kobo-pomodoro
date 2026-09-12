#!/bin/sh
# Starts Cobalt. The stock reader is stopped, the panel is handed over, and the
# reader is started again when the session ends. A reboot always returns to the
# stock reader, so nothing here needs undoing by hand.
set -e
root=/mnt/onboard/.adds/cobalt
# `kobo setup --enable-ssh` leaves only a public key here. The reader's
# root-owned menu action can finish the step a USB volume cannot.
staged_key="$root/bootstrap/authorized_key"
if [ -s "$staged_key" ]; then
  umask 077
  # Root's home is not /root on every Kobo: the i.MX6 firmware ships
  # root:...:0:0:root:/:/bin/sh, and sshd resolves AuthorizedKeysFile
  # relative to the home directory, so a key under /root never authenticates
  # there. Ask /etc/passwd instead of assuming.
  home=$(awk -F: '$1 == "root" { print $6 }' /etc/passwd)
  home="${home%/}"
  keys="$home/.ssh/authorized_keys"
  mkdir -p "$home/.ssh"
  touch "$keys"
  chmod 700 "$home/.ssh"
  chmod 600 "$keys"
  key=$(head -n 1 "$staged_key")
  found=false
  while IFS= read -r known; do
    if [ "$known" = "$key" ]; then
      found=true
      break
    fi
  done < "$keys"
  if [ "$found" = false ]; then
    printf '%s\n' "$key" >> "$keys"
  fi
  rm -f "$staged_key"
  sync
fi
# The terminal opens a pty: ptsname names /dev/pts/N and the child opens it.
# Some Kobo firmware does not mount devpts, so without this every terminal is
# refused with Failed. A kernel mount, not a write to the root filesystem, and
# gone again at the next reboot.
if ! grep -q ' /dev/pts ' /proc/mounts 2>/dev/null; then
  mkdir -p /dev/pts 2>/dev/null &&
    mount -t devpts devpts /dev/pts -o mode=0620,ptmxmode=0666 || true
fi
KOBO_PRESENT_UNLOCK=OWNER_ATTENDED_PANEL_SESSION \
  exec "$root/bin/kobod" --present "$root/bin/kobo-launcher" > /mnt/onboard/kobod.txt 2>&1

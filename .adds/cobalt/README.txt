Cobalt
======

Everything is on the same partition your books are on and is visible from any
computer over USB. The managed payload is in this folder; its stable launch
entrypoint is the sibling .adds/cobalt-launch.sh.

For a safe complete removal, run `kobo setup --undo`. It removes the managed
cobalt/current, next, and previous trees, .adds/cobalt-launch.sh, and only the
exact Cobalt entry from .adds/nm/cobalt or .adds/nm/menu. Owner folders are
moved to .adds/cobalt.recovery.N first. Inspect those directories and any
.adds/cobalt.unusable[.N] quarantine before deleting recoverable data.
Nothing was written to the system partition and no startup script was added.

To start it: run .adds/cobalt-launch.sh. If you have NickelMenu installed, add
this one line to .adds/nm/menu to get an entry in the reader's own menu:

  menu_item :main    :Cobalt    :cmd_spawn    :quiet:/mnt/onboard/.adds/cobalt-launch.sh

Starting Cobalt stops the stock reader for the length of the session and
starts it again afterwards. That takes twenty to thirty seconds each way. A
reboot always returns you to the stock reader.

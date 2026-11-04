#!/usr/bin/env python3
"""Remove a font family from OpenPencil's downloaded-font cache.

When a font isn't installed, OpenPencil downloads a Google Fonts subset with only the characters
it needs. Subsets renumber their glyphs, so once a second copy of the family is loaded (another
subset, or the installed font), text is shaped with one copy and drawn with another: "Budget"
turns into "Bt dgeß". Install the family locally, quit OpenPencil, and run this to drop the
subsets, so the installed font is the only copy left:

    brew install --cask font-familjen-grotesk
    scripts/openpencil-purge-font.py "Familjen Grotesk"

The manifest is backed up next to itself first. Other families are left alone.
"""

import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

CACHE = Path.home() / "Library/Application Support/net.dannote.open-pencil/cache/v1/font-cache/v1"


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__.strip().splitlines()[0])
        print('Usage: openpencil-purge-font.py "Family Name"')
        return 2
    family = sys.argv[1].strip().casefold()

    running = subprocess.run(["pgrep", "-x", "OpenPencil"], capture_output=True).returncode == 0
    if running:
        print("OpenPencil is running. Quit it first (⌘Q), or it keeps the subsets in memory.")
        return 1

    manifest_path = CACHE / "manifest"
    if not manifest_path.exists():
        print(f"No font cache at {CACHE}. Nothing to do.")
        return 0

    manifest = json.loads(manifest_path.read_text())
    entries = manifest["value"]["entries"]
    doomed = {k: v for k, v in entries.items() if v.get("family", "").casefold() == family}
    if not doomed:
        print(f'No cached copies of "{sys.argv[1]}".')
        return 0

    backup = manifest_path.with_name(f"manifest.{time.strftime('%Y%m%d-%H%M%S')}.bak")
    shutil.copy2(manifest_path, backup)

    freed = 0
    for key, entry in doomed.items():
        file = CACHE / "files" / entry["file"]
        if file.exists():
            freed += file.stat().st_size
            file.unlink()
        del entries[key]
    manifest["updatedAt"] = int(time.time() * 1000)
    manifest_path.write_text(json.dumps(manifest, separators=(",", ":")))

    print(f'Removed {len(doomed)} cached copies of "{sys.argv[1]}" ({freed // 1024} KB).')
    print(f"Manifest backup: {backup}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

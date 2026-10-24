"""Changes the app version everywhere a release needs it.

    python tools/bump.py 1.2.10 1.2.11

package.json, package-lock.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml and Cargo.lock, the README's
release steps and publish-update.yml's KEEP_RELEASE_JSON_UNTIL. Stops without changing anything if a file doesn't
say the old version where expected.
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main(old: str, new: str) -> None:
    edits = {
        ".github/workflows/publish-update.yml": [(f"KEEP_RELEASE_JSON_UNTIL: '{old}'", f"KEEP_RELEASE_JSON_UNTIL: '{new}'", 1)],
        "README.md": [(f"releases up to {old},", f"releases up to {new},", 1), (f"git tag v{old} && git push origin v{old}", f"git tag v{new} && git push origin v{new}", 1)],
        "package.json": [(f'"version": "{old}"', f'"version": "{new}"', 1)],
        "package-lock.json": [(f'"version": "{old}"', f'"version": "{new}"', 2)],
        "src-tauri/tauri.conf.json": [(f'"version": "{old}"', f'"version": "{new}"', 1)],
        "src-tauri/Cargo.toml": [(f'version = "{old}"', f'version = "{new}"', 1)],
        "src-tauri/Cargo.lock": [(f'name = "resincraft"\nversion = "{old}"', f'name = "resincraft"\nversion = "{new}"', 1)],
    }
    changed = {}
    for rel, pairs in edits.items():
        text = (ROOT / rel).read_text(encoding="utf-8")
        for a, b, count in pairs:
            found = text.count(a)
            if found != count:
                sys.exit(f"{rel}: expected {count}x {a!r}, found {found}. Nothing was changed.")
            text = text.replace(a, b)
        changed[rel] = text
    for rel, text in changed.items():
        (ROOT / rel).write_text(text, encoding="utf-8", newline="")
    print(f"{old} -> {new} in {len(changed)} files")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])

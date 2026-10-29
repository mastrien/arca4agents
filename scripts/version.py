"""Versão única do Arca for Agents em todos os pacotes.

    python scripts/version.py              # mostra a versão de cada arquivo
    python scripts/version.py 0.4.0        # grava 0.4.0 em todos
    python scripts/version.py --check v0.4.0   # CI: falha se algum arquivo não tiver essa versão

No Android, o versionCode é derivado (MAJOR*10000 + MINOR*100 + PATCH) e precisa crescer a cada
release, senão o instalador recusa a atualização.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CARGO = ROOT / "Cargo.toml"
TAURI = ROOT / "apps/desktop/src-tauri/tauri.conf.json"
PACKAGE = ROOT / "apps/desktop/package.json"
PUBSPEC = ROOT / "apps/mobile/pubspec.yaml"

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")


def read_all() -> dict[str, str]:
    cargo = re.search(r'\[workspace\.package\]\s*\nversion = "([^"]+)"', CARGO.read_text(encoding="utf-8"))
    pubspec = re.search(r"^version:\s*([^+\s]+)", PUBSPEC.read_text(encoding="utf-8"), re.M)
    return {
        "Cargo.toml": cargo.group(1) if cargo else "?",
        "tauri.conf.json": json.loads(TAURI.read_text(encoding="utf-8"))["version"],
        "package.json": json.loads(PACKAGE.read_text(encoding="utf-8"))["version"],
        "pubspec.yaml": pubspec.group(1) if pubspec else "?",
    }


def version_code(version: str) -> int:
    major, minor, patch = (int(p) for p in version.split("."))
    return major * 10000 + minor * 100 + patch


def write_json_version(path: Path, version: str) -> None:
    data = json.loads(path.read_text(encoding="utf-8"))
    data["version"] = version
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def write_all(version: str) -> None:
    text = CARGO.read_text(encoding="utf-8")
    text = re.sub(r'(\[workspace\.package\]\s*\nversion = ")[^"]+(")', rf"\g<1>{version}\g<2>", text)
    CARGO.write_text(text, encoding="utf-8")
    write_json_version(TAURI, version)
    write_json_version(PACKAGE, version)
    text = PUBSPEC.read_text(encoding="utf-8")
    text = re.sub(r"^version:.*$", f"version: {version}+{version_code(version)}", text, count=1, flags=re.M)
    PUBSPEC.write_text(text, encoding="utf-8")


def main(args: list[str]) -> int:
    if not args:
        for name, version in read_all().items():
            print(f"{name:16} {version}")
        return 0
    if args[0] == "--check":
        expected = args[1].removeprefix("v")
        wrong = {k: v for k, v in read_all().items() if v != expected}
        for name, version in wrong.items():
            print(f"{name}: {version} (esperado {expected})", file=sys.stderr)
        return 1 if wrong else 0
    version = args[0].removeprefix("v")
    if not SEMVER.match(version):
        print(f"Versão inválida: {version} (use MAJOR.MINOR.PATCH)", file=sys.stderr)
        return 1
    write_all(version)
    for name, value in read_all().items():
        print(f"{name:16} {value}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

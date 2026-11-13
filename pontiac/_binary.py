"""Finds (or builds) the pontiac command-line program."""
import os
import platform
import shutil
import subprocess
from pathlib import Path

_HERE = Path(__file__).resolve().parent
_REPO = _HERE.parent
_cached = None


def _works(path):
    try:
        out = subprocess.run([str(path), "version"], capture_output=True, text=True, timeout=20)
        return out.returncode == 0 and out.stdout.startswith("pontiac")
    except (OSError, subprocess.SubprocessError):
        return False


def _build():
    """Builds the program from source with cargo (needs Rust; crates come from crates.io)."""
    cli = _REPO / "cli"
    if not (cli / "Cargo.toml").exists() or shutil.which("cargo") is None:
        return None
    subprocess.run(["cargo", "build", "--release"], cwd=cli, check=True)
    exe = cli / "target" / "release" / ("pontiac.exe" if os.name == "nt" else "pontiac")
    return exe if exe.exists() else None


def binary():
    """Path of a working pontiac program. Order: $PONTIAC_BIN, the bundled Linux build,
    a local cargo build, then building from source."""
    global _cached
    if _cached:
        return _cached
    candidates = []
    if os.environ.get("PONTIAC_BIN"):
        candidates.append(Path(os.environ["PONTIAC_BIN"]))
    if platform.system() == "Linux" and platform.machine() in ("x86_64", "AMD64"):
        candidates.append(_HERE / "bin" / "pontiac-linux-x86_64")
    candidates.append(_REPO / "cli" / "target" / "release" / ("pontiac.exe" if os.name == "nt" else "pontiac"))
    for c in candidates:
        if c.exists():
            if os.name != "nt" and not os.access(c, os.X_OK):
                try:
                    c.chmod(c.stat().st_mode | 0o111)
                except OSError:
                    pass
            if _works(c):
                _cached = str(c)
                return _cached
    built = _build()
    if built and _works(built):
        _cached = str(built)
        return _cached
    raise RuntimeError(
        "No working pontiac program found. On Linux x86-64 the bundled one should run as is; "
        "otherwise install Rust and run `cargo build --release` in the cli folder, "
        "or point PONTIAC_BIN at a built program."
    )

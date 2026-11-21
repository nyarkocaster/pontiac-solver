"""`python -m pontiac ...` or the `pontiac` command: passes straight through to the solver program.
Three extra words are handled here: `selftest [--full]`, `guide` and `examples`."""
import json
import subprocess
import sys

from ._binary import binary


def main():
    args = sys.argv[1:]
    if args[:1] == ["selftest"]:
        import pontiac
        r = pontiac.selftest(full="--full" in args)
        print(json.dumps(r, indent=2))
        sys.exit(0 if r["ok"] else 1)
    if args[:1] == ["guide"]:
        import pontiac
        print(pontiac.guide())
        return
    if args[:1] == ["examples"]:
        import pontiac
        print(json.dumps(pontiac.examples()))
        return
    sys.exit(subprocess.call([binary(), *args]))


if __name__ == "__main__":
    main()

"""`python -m pontiac ...` or the `pontiac` command: passes straight through to the solver program."""
import subprocess
import sys

from ._binary import binary


def main():
    sys.exit(subprocess.call([binary(), *sys.argv[1:]]))


if __name__ == "__main__":
    main()

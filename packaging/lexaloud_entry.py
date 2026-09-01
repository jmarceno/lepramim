"""PyInstaller entry point for the CPU AppImage.

The source distribution keeps the normal setuptools console script.  The
AppImage uses this tiny entry point so PyInstaller can freeze the same CLI
without relying on a venv shebang that points at the build machine.
"""

from lexaloud.cli import main

if __name__ == "__main__":
    raise SystemExit(main())

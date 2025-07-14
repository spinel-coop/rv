# UV Notes

See the `uv` implementation in `~/Development/github.com/astral-sh/uv`

## python discovery

⏺ uv finds Python installations through a sophisticated discovery system in uv-python/src/discovery.rs:944. It searches multiple sources
  in priority order:

  1. Parent interpreter (when run from within Python)
  2. Virtual environments (active, conda, discovered .venv dirs)
  3. Managed installations (uv-installed Pythons in ~/.local/share/uv/python/)
  4. System PATH (various executable names like python3.x, pypy)
  5. Windows registry (PEP 514 compliant)
  6. Microsoft Store (Windows only)

  The core function find_python_installations() generates candidate paths, queries each executable for interpreter info, then filters by
  version/implementation requirements. Each installation includes source metadata and is uniquely identified by implementation, version,
  OS, architecture, and libc.

### metadata storage

⏺ The metadata is stored in two main ways:

1. Interpreter Metadata Cache
  - Location: ~/.cache/uv/interpreter-v4/ (or equivalent XDG cache dir)
  - Format: MessagePack files (.msgpack)
  - Key: Hash of executable path
  - Invalidation: Based on executable modification time
  - Contains: Platform info, installation schemes, system paths, version details
2. Managed Python Installations
  - Location: ~/.local/share/uv/python/ (or equivalent XDG data dir)
  - Structure: cpython-3.12.0-x86_64-linux-gnu/ (using PythonInstallationKey)
  - Contains: Full Python installation files

  Important: Discovery results themselves are not cached - uv re-discovers available Pythons each time. Only the expensive interpreter
  metadata queries are cached to avoid repeatedly running Python subprocesses.

  The cache structure is defined in uv-python/src/interpreter.rs:996 and uses `CachedByTimestamp<InterpreterInfo>` to ensure cache
  invalidation when executables change.

## PythonInstallationKey

⏺ Based on the code, PythonInstallationKey contains these fields:

  - implementation: LenientImplementationName (e.g., "cpython", "pypy", "graalpy")
  - major: u8 - Major version number
  - minor: u8 - Minor version number
  - patch: u8 - Patch version number
  - prerelease: Option<Prerelease> - Optional prerelease info (alpha, beta, rc)
  - os: Os - Operating system (Linux, macOS, Windows)
  - arch: Arch - CPU architecture (x86_64, aarch64, etc.)
  - libc: Libc - C library variant (glibc, musl, etc.)
  - variant: PythonVariant - Python variant (default or free-threaded)

  This creates unique identifiers like cpython-3.12.0-x86_64-linux-gnu for directory names and ensures each Python installation can be
  uniquely identified by its implementation, version, and platform characteristics.

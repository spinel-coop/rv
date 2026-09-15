# rv doctor

Checks the current environment for anything that would stop `rv` from working,
and prints the command that fixes each problem it finds.

`rv doctor` is offline and read-only. It makes no network requests and changes
nothing on disk, so it is safe to run anywhere, including in CI.

## How it works

Most of the report comes from a single comparison. `rv` already knows what a
correct environment looks like — it is what `rv shell env` hands to the shell
integration on every prompt. `doctor` computes that same environment and diffs
it against the one the `rv` process actually inherited. Anything that differs is
reported, along with the command that puts it back.

That means the report is derived from rv's real environment handling rather than
from a separate list of assumptions, so the two cannot drift apart.

The checks fall into three groups:

**Environment** — which Ruby rv resolves to and why, whether it is installed,
whether an rv-managed Ruby is active in this shell, whether `RUBY_ROOT` still
points at a Ruby that exists, whether each variable rv exports matches what it
would export now, whether rv's directories are on `PATH`, and whether `ruby`
resolves to the interpreter rv selected.

**Installation** — whether the running `rv` is the same one a shell would find
on `PATH`, and whether rv's Ruby and cache directories are writable.

**Configuration** — whether `rv.kdl` and bundler's settings parse, and whether
more than one version file (`.ruby-version`, `.tool-versions`, `Gemfile.lock`)
pins a different Ruby.

## Output

```
$ rv doctor

Environment
  ✔ ruby              ruby-3.4.1 — Default version pinned by .ruby-version
  ✔ RUBY_ROOT         ~/.local/share/rv/rubies/ruby-3.4.1
  ✔ PATH              rv's directories are present
  ✗ ruby on PATH      resolves to ~/.rbenv/shims/ruby
                      rbenv is ahead of rv on PATH. Remove its setup from your
                      shell startup files, or move rv's after it.
  ! RUBYOPT           set, but rv clears it
                      Usually left behind by another Ruby manager. It leaks into
                      everything rv runs.
                      → unset RUBYOPT

Installation
  ✔ rv                ~/.local/bin/rv
  ✔ ruby directory    ~/.local/share/rv/rubies
  ✔ cache directory   ~/.cache/rv

Configuration
  ✔ rv settings       defaults
  ✔ bundler settings  defaults

1 check failed, 1 warning.
```

`✗` is a failure and exits non-zero. `!` is a warning: it works today, but will
cause trouble later, and does not affect the exit code.

## Options

`--format json` emits the checks as an array of objects, each with `section`,
`name`, `status` (`ok`, `warn`, `fail`, or `skipped`), `message`, and an optional
`fix` holding a `detail` and sometimes a `command`. The JSON shape is the stable
interface for scripting; the text layout may change.

`--exit-zero` prints the report but always exits successfully, for pipelines that
want the diagnosis without failing the step.

## Limitations

`rv doctor` only sees variables that were exported. It cannot see shell state
that was never exported, and it cannot tell whether rv's environment was set by
the shell integration or by an enclosing `rv run` — only whether it is present
and current.

# `rv` changelog

## rv 0.3.0

Highlights include support for Ruby versions 3.2.x, 3.5.0-preview1, 4.0.0-preview2, automatic Ruby installation during the `rv ruby run` command, and support for `.tool-versions` files, which are also used by the [asdf](https://asdf-vm.com) and [mise](https://mise.jdx.dev) tools.

- Support Ruby 0.49 (<https://github.com/spinel-coop/rv/pull/189>, @indirect)
- Install Ruby during `rv ruby run` (<https://github.com/spinel-coop/rv/pull/182>, @adamchalmers)
- Sign macOS binaries (<https://github.com/spinel-coop/rv/pull/179>, @indirect)
- Support SOCKS Proxy configuration (<https://github.com/spinel-coop/rv/pull/175>, @indirect)
- Support `.tool-versions` file for Ruby version (<https://github.com/spinel-coop/rv/pull/172>, @indirect)
- Add configuration for Nushell (<https://github.com/spinel-coop/rv/pull/170>, @indirect)
- Fix fish auto-switch when shell launches (<https://github.com/spinel-coop/rv/pull/165>, @Mado13)
- Add `rv cache prune` (<https://github.com/spinel-coop/rv/pull/164>, @savechina)
- Add `rv ruby uninstall` (<https://github.com/spinel-coop/rv/pull/153>, @savechina)
- Add `rv ruby dir` (<https://github.com/spinel-coop/rv/pull/150>, @savechina)

## rv 0.2.0

Adds x86 macOS as a supported platform, Ruby 3.3, and YJIT for all Rubies. Adds shells bash, fish, and nushell.

- Build for x86 macOS (<https://github.com/spinel-coop/rv/pull/137>, @indirect)
- Enable Actions support for merge queue (<https://github.com/spinel-coop/rv/pull/137>, @indirect)
- Test all four platforms (<https://github.com/spinel-coop/rv/pull/136>, @adamchalmers)
- Set GEM_PATH to gem location, no bin (<https://github.com/spinel-coop/rv/pull/110>, @Thomascountz)
- Default data to `~/.local/share/rv`, respect XDG (<https://github.com/spinel-coop/rv/pull/84>, @lgarron)
- Support Ruby 3.3.x via the new rv-ruby release scheme (<https://github.com/spinel-coop/rv/pull/132>, @adamchalmers)
- Support nushell (<https://github.com/spinel-coop/rv/pull/123>, @adamchalmers)
- Add code coverage reporting to CI (<https://github.com/spinel-coop/rv/pull/112>, @adamchalmers)
- Add benchmarks and fuzz testing for library code (<https://github.com/spinel-coop/rv/pull/95>, @adamchalmers)
- Shell completions for zsh, bash, and fish (<https://github.com/spinel-coop/rv/pull/81>, @lgarron)
- Fix race condition from tracing+rayon together (<https://github.com/spinel-coop/rv/pull/73>, @segiddins)
- List available Rubies (<https://github.com/spinel-coop/rv/pull/68>, @coezbek)
- Support fish shell (<https://github.com/spinel-coop/rv/pull/67>, @renatolond)
- Compact top-level help, even on `--help` (<https://github.com/spinel-coop/rv/pull/70>, @coezbek)

## rv 0.1.1

- Dual license MIT / Apache-2
- turn up LTO for smaller releases
- Fix `rv ruby pin` if not run in a project_dir
- Fix `bin/setup` in non-root situations under Ubuntu
- replace openssl with rustls
- Added bash to supported shells

## rv 0.1.0

First release. Supports macOS 14+, Ubuntu 24.04+, Ruby 3.4.1+, and zsh.

- `rv ruby` command group
  - `rv ruby list` command to show installed rubies
  - `rv ruby install` to install precompiled rubies
  - `rv ruby run` to run a specific ruby version regardless of `.ruby-version`
- `rv shell` command group
  - `rv shell init` command to set up automatic ruby switching that respects `.ruby-version` files

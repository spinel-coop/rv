# `rv` changelog

## rv 0.2.0

Adds x86 macOS as a supported platform, Ruby 3.3, and YJIT for all Rubies. Adds shells bash, fish, and nushell.

- Build for x86 macOS (#137, @indirect)
- Enable Actions support for merge queue (#137, @indirect)
- Test all four platforms (#136, @adamchalmers)
- Set GEM_PATH to gem location, no bin (#110, @Thomascountz)
- Default data to `~/.local/share/rv`, respect XDG (#84, @lgarron)
- Support new rv-ruby release scheme (#132, @adamchalmers)
- Support nushell (#123, @adamchalmers)
- Add code coverage reporting to CI (#112, @adamchalmers)
- Add benchmarks and fuzz testing for library code (#95, @adamchalmers)
- Shell completions for zsh, bash, and fish (#81, @lgarron)
- Fix race condition from tracing+rayon together (#73, @segiddins)
- List available Rubies (#68, @coezbek)
- Support fish shell (#67, @renatolond)
- Compact top-level help, even on `--help` (#70, @coezbek)

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

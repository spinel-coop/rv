# `rv` changelog

## rv 0.4.1 (9 January, 2025)

This is a small release to fix a few bugs in the 0.4.0 release.

 - `rv ruby` commands should act on `rv` rubies before rubies from other sources (#333, @deivid-rodriguez)
 - `rv ci` command should not be hidden (#337, @adamchalmers)
 - Improvements to how `rv ruby list` displays active ruby (#339, @deivid-rodriguez)
 - Clearer descriptions for `rv ruby` subcommands (#338, @sstephenson)
 - Tests: Add smoke tests for `rv ci` with popular projects (#331, @case)

## rv 0.4.0 (6 January, 2025)

Major new command: `rv clean-install`. Similar to `bundle install --frozen`, you can use `rv ci` to install gems that have already been resolved and recorded in your `Gemfile.lock`. If you notice anything wrong when installing your gems, please [let us know](https://github.com/spinel-coop/rv/issues/new)!

Improves listing Ruby versions, hiding (uninstalled) pre-releases once a final is out.

- Hide 3.5.0-preview1 in `rv ruby list` (<https://github.com/spinel-coop/rv/pull/270>, @deivid-rodriguez)
- Install submodules for git sources (<https://github.com/spinel-coop/rv/pull/274>, @adamchalmers)
- Download gems in parallel (<https://github.com/spinel-coop/rv/pull/275>, @adamchalmers)
- Install Ruby once for `ci` (<https://github.com/spinel-coop/rv/pull/278>, @adamchalmers)
- Install path gems (<https://github.com/spinel-coop/rv/pull/282>, @indirect)
- Fix dep graph race condition (<https://github.com/spinel-coop/rv/pull/309>, @case)
- Compile gems in dependency order (<https://github.com/spinel-coop/rv/pull/295>, @indirect)
- Integrate tracing with Instruments.app (<https://github.com/spinel-coop/rv/pull/301>, @adamchalmers)
- Parse branches and tags for git gems (<https://github.com/spinel-coop/rv/pull/313>, @case)
- Add `Version::from(segments)` (<https://github.com/spinel-coop/rv/pull/320>, @kaspth)
- Separate `RubyVersion` from `RubyRequest` (<https://github.com/spinel-coop/rv/pull/322>, @adamchalmers)

## rv 0.3.1 (30 December, 2025)

Ruby 4.0.0 is now available! (and has been since Christmas Day). This release is mainly a fix to make sure that previews are considered "before" final releases, so we will stop showing 4.0.0-preview3 as if you might want to install it after 4.0.0.

- Parse submodules option on git sources (<https://github.com/spinel-coop/rv/pull/268>, @adamchalmers)
- Sort preview releases before final releases (<https://github.com/spinel-coop/rv/pull/266>, @deivid-rodriguez)
- Default to true XDG spec dir `~/.local/share` (<https://github.com/spinel-coop/rv/pull/265>, @deivid-rodriguez)
- Use final Ruby versions in `rv ruby run`, not preview versions (<https://github.com/spinel-coop/rv/pull/258>, @deivid-rodriguez)
- Print shell integration instructions on e.g. `rv shell zsh` (<https://github.com/spinel-coop/rv/pull/255>, @deivid-rodriguez)
- Update Ruby version for every command, not just `cd` (<https://github.com/spinel-coop/rv/pull/250>, @deivid-rodriguez)
- Set MANPATH for Ruby man pages (<https://github.com/spinel-coop/rv/pull/251>, @case)
- Use the latest Ruby in `rv ruby run` by default (<https://github.com/spinel-coop/rv/pull/237>, @deivid-rodriguez)
- Set GEM_HOME to `~/.local/share/rv/gems` if `~/.gems` doesn't exist (<https://github.com/spinel-coop/rv/pull/231>, @lgarron)
- Ensure `rv ruby pin` prints versions consistently (<https://github.com/spinel-coop/rv/pull/229>, @deivid-rodriguez)

## rv 0.3.0 (8 December, 2025)

Highlights include support for Ruby versions 3.2.x, 3.5.0-preview1, 4.0.0-preview2, automatic Ruby installation during the `rv ruby run` command, and support for `.tool-versions` files, which are also used by the [asdf](https://asdf-vm.com) and [mise](https://mise.jdx.dev) tools.

Just for fun, we also added the oldest version of Ruby that has released source code: 0.49! If you'd like to try it, there are some [example scripts](https://github.com/sampersand/ruby-0.49?tab=readme-ov-file#sample-program) available in the [repo where @sampersand modernized the code](https://github.com/sampersand/ruby-0.49).

Special thanks to our new contributors @savechina, @Mado13, @case, and @deivid-rodriguez!

- Make `ruby pin` reject invalid versions (<https://github.com/spinel-coop/rv/pull/223>, @deivid-rodriguez)
- Test against Arch Linux (<https://github.com/spinel-coop/rv/pull/193>, @case)
- Support Ruby preview releases (<https://github.com/spinel-coop/rv/pull/201>, @indirect)
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

## rv 0.2.0 (8 October, 2025)

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

## rv 0.1.1 (28 August, 2025)

- Dual license MIT / Apache-2
- turn up LTO for smaller releases
- Fix `rv ruby pin` if not run in a project_dir
- Fix `bin/setup` in non-root situations under Ubuntu
- replace openssl with rustls
- Added bash to supported shells

## rv 0.1.0 (25 August, 2025)

First release. Supports macOS 14+, Ubuntu 24.04+, Ruby 3.4.1+, and zsh.

- `rv ruby` command group
  - `rv ruby list` command to show installed rubies
  - `rv ruby install` to install precompiled rubies
  - `rv ruby run` to run a specific ruby version regardless of `.ruby-version`
- `rv shell` command group
  - `rv shell init` command to set up automatic ruby switching that respects `.ruby-version` files

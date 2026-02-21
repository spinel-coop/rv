# `rv`, a very fast Ruby gem and project manager

Manage your projects, including all their Ruby versions and gems, with `rv`.

- Run any command or script that needs ruby with `rv run`.
- Install gem CLIs in isolated environments with `rv tool install`.
- Run any gem command immediately, like `rvx rails new myapp`.
- Install project gems with `rv clean-install`.

## Install

On macOS or Linux, install `rv` with Homebrew:

```bash
brew install rv
```

Alternatively, use the standalone installer script:

```bash
curl -LsSf https://github.com/spinel-coop/rv/releases/latest/download/rv-installer.sh | sh
```

On Windows, open PowerShell as Administrator and run:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/spinel-coop/rv/releases/latest/download/rv-installer.ps1 | iex"
```

**Note: On Windows PowerShell, use `rvw` instead of `rv` because `rv` is an alias for the built-in `Remove-Variable`.**

For other installation options or to download specific versions manually, visit the [releases page](https://github.com/spinel-coop/rv/releases).

## Quickstart

```bash
rv run [ruby|irb|myscript.rb] # run any command or script with Ruby available
rv tool install rerun # install CLI tools into dedicated environments
rvx rails new . # run any gem CLI directly
rv clean-install # install project Ruby and gems from Gemfile.lock
rv ruby pin 4.0.0 # set a project Ruby version
rv ruby install 4.0.0 # install a Ruby version
rv shell [zsh|bash|fish|nu|powershell] # set up automatic version switching
```

On Windows PowerShell, use `rvw` instead of `rv`:

```powershell
rvw run ruby             # run any command or script with Ruby available
rvw clean-install        # install project Ruby and gems from Gemfile.lock
rvw shell powershell     # set up automatic version switching
```

See [SHELL INTEGRATION](docs/SHELL_INTEGRATION.md) for more about `.ruby-version` and automatic version switching.

## Install Ruby in under 2 seconds

![rv installs ruby 3.4.7 in 1.8 seconds](docs/img/rv-ruby-install.svg)

For details, see [INSTALL_BENCHMARK.md](docs/INSTALL_BENCHMARK.md).

## Testimonials

"what the heckie that just installed a ruby version for me in .22 seconds???"
 &mdash; [c. ruiyi smallbird](https://bsky.app/profile/veryroundbird.house/post/3lxdwtsbwp22o)

"Holy wow that thing \_flies\_."
 &mdash; [Andrea Fomera](https://bsky.app/profile/afomera.dev/post/3m4wjfvqrhk2h)

## Requirements

- **Operating Systems**: macOS 14+, Linux glibc 2.35+, Windows 10+
- **Architectures**: x86, arm64
- **Ruby Versions**: Ruby 3.2, 3.3, 3.4, and 4.0.
- **Shells**: zsh, bash, fish, nushell, PowerShell. See [SHELL INTEGRATION](docs/SHELL_INTEGRATION.md) for more.

## From Spinel Cooperative

[Spinel.coop](https://spinel.coop) can bring your team an obsession with developer productivity and decades of experience from the core teams of Rails, Hotwire, Bundler, and rbenv. [Book a free call with us](https://savvycal.com/spinel/client) to get started today.

## Community

Join us in [discussions on GitHub](https://github.com/spinel-coop/rv/discussions), or the `#rv` channel in the the [Bundler Slack](https://bundler.slack.com) and the [Spinel Discord](https://discord.gg/5EG2pCad).

## Plans

All-in-one tooling for Ruby developers.

- Install & manage all Ruby versions.
- Manage gems, applications, CLI tools, and scripts, effortlessly.
- Handle everything Ruby provided by `rvm`, `rbenv`, `chruby`, `asdf`, `mise`, `ruby-build`, `ruby-install`, `bundler`, and `rubygems`, all at once.
- Install Ruby and all your gems in seconds, without having to compile anything ever again.
- Install `ruby-head` versions for easier development for Ruby core contributors.
- `mise` plugin, so you get fast no-fuss Ruby installs too.

### Future features

- Run any script, installing all needed gems, like `rv run script.rb`.
- Manage project gems with `rv install`, `rv add`, and `rv remove`.
- Create gems with `rv gem`, and publish them with `rv publish`.

See [PLANS.md](docs/PLANS.md) for more on our future plans.

## Contributing

Install dependencies on macOS or Ubuntu with `bin/setup`. Make changes, and then run the development binary with `bin/rv`, or install the development binary to your system with `bin/install`.

On Windows, equivalent PowerShell scripts are available in `bin\powershell\`.

When you're done, make sure to run the tests with `bin/test`, and the linter with `bin/lint`. Then, send us a pull request! We love pull requests.

## FAQ

**Does rv have a website?**

Yes! You're looking at it. You can reach this page with the URL [rv.dev](https://rv.dev).

**Why are you doing this?**

[@indirect](https://github.com/indirect), long-time project lead for [Bundler](https://bundler.io) and [RubyGems](https://rubygems.org), wrote [a blog post about his motivation to create `rv`](https://andre.arko.net/2025/08/25/rv-a-new-kind-of-ruby-management-tool/).

**How do you pronounce rv?**

"arr vee", the names of the two letters, spelled out.

## Acknowledgements

`rv` is heavily inspired by [`uv`](https://github.com/astral-sh/uv), as you might guess from the name.

It also takes inspiration, features, and approaches from [Bundler](https://bundler.io), [Frum](https://github.com/TaKO8Ki/frum), [Homebrew](https://brew.sh), [npm](https://npmjs.com), [Cargo](https://github.com/rust-lang/cargo), and [Orogene](https://github.com/orogene/orogene).

We'd also like to thank everyone who has ever worked on a package manager in the past for helping get all of us to where we are today.

## Similar Projects

- [Bundler](https://bundler.io) manages project gems.
- [Ruby Butler](https://github.com/RubyElders/ruby-butler) manages project tasks, Rubies, and gems.

## License

`rv` is licensed under either [Apache-2.0](/LICENSE-APACHE) or [MIT](/LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion by you, as defined in the Apache-2.0 license, shall be dually licensed as above, without any additional terms or conditions.

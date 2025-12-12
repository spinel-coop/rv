# Meet `rv`, the super fast no-fuss Ruby version manager

Welcome to `rv`, the Ruby version manager that revs your Ruby installs so [they take just seconds](#install-benchmark).

[In the future](#future-features), rv can also manage your tools, gems, and packages, faster than you would believe is possible.

`rv` is greatly inspired by & builds on top of tons of work done by other package managers & their maintainers. See our [acknowledgements](#acknowledgements).

## Why `rv`

![rv installs ruby 3.4.7 in 1.8 seconds](docs/img/rv-ruby-install.svg)

We precompile Ruby 3.2+ for [macOS & Linux](#requirements) to vastly improve install times & make a number of installation issues a thing of the past.

- **Super fast install**: rv installs Ruby 3.2+ in seconds.
- **Unbreakable**: Homebrew updating OpenSSL will never break your Ruby install again.
- **Zero compile time**: no more waiting 5-40min to compile Ruby on your dev machine or deployment dyno.
- **Zero compile errors**: your Ruby install won't fail to compile due to low-level libraries being missing or unlinked.
- **Eco-friendly**: saving minutes of compilation time across thousands of dev machines and deployment dynos across Ruby releases all add up.

### Install Benchmark

Here's the install time difference running on a base M5 MacBook Pro in Low Power Mode and High Power Mode:

| | seconds | times slower |
| ---: | ---: | ---: |
| rv | 2.479 | 1.0x |
| ruby-build (High Power) | 89.611 | 36.14x |
| ruby-build (Low Power) | 159.752 | 64.44x |

We happen to be using `rbenv` here, but `chruby`, `mise` and other tools all use `ruby-build` under the hood to compile Ruby during install.

`rv` install time is constrained by network speed so your mileage will vary. This was run from Copenhagen, Denmark on a reasonably fast connection.

<details>
  <summary>See the benchmark commands we ran</summary>

#### rv install time

```bash
time rv ruby install 3.4.7
Downloaded https://github.com/spinel-coop/rv-ruby/releases/latest/download/ruby-3.4.7.arm64_sonoma.tar.gz to ~/.cache/rv/ruby-v0/tarballs/8758fed525bd3750.tar.gz
Installed Ruby version ruby-3.4.7 to ~/.data/rv/rubies

real 0m2.479s
user 0m0.362s
sys  0m0.551s
```

#### rbenv + ruby-build install in High Power Mode

```bash
time rbenv install 3.4.7
ruby-build: using openssl@3 from homebrew
==> Downloading ruby-3.4.7.tar.gz...
-> curl -q -fL -o ruby-3.4.7.tar.gz https://cache.ruby-lang.org/pub/ruby/3.4/ruby-3.4.7.tar.gz
  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current
                                 Dload  Upload   Total   Spent    Left  Speed
100 22.1M  100 22.1M    0     0  11.9M      0  0:00:01  0:00:01 --:--:-- 11.9M
==> Installing ruby-3.4.7...
ruby-build: using libyaml from homebrew
ruby-build: using gmp from homebrew
-> ./configure "--prefix=$HOME/.rbenv/versions/3.4.7" --with-openssl-dir=/opt/homebrew/opt/openssl@3 --enable-shared --with-libyaml-dir=/opt/homebrew/opt/libyaml --with-gmp-dir=/opt/homebrew/opt/gmp --with-ext=openssl,psych,+
-> make -j 10
-> make install
==> Installed ruby-3.4.7 to ~/.rbenv/versions/3.4.7

real 1m29.611s
user 2m54.163s
sys  0m57.157s
```

#### rbenv + ruby-build install in Low Power Mode

```bash
time rbenv install 3.4.7
ruby-build: using openssl@3 from homebrew
==> Downloading ruby-3.4.7.tar.gz...
-> curl -q -fL -o ruby-3.4.7.tar.gz https://cache.ruby-lang.org/pub/ruby/3.4/ruby-3.4.7.tar.gz
  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current
                                 Dload  Upload   Total   Spent    Left  Speed
100 22.1M  100 22.1M    0     0  6721k      0  0:00:03  0:00:03 --:--:-- 6719k
==> Installing ruby-3.4.7...
ruby-build: using libyaml from homebrew
ruby-build: using gmp from homebrew
-> ./configure "--prefix=$HOME/.rbenv/versions/3.4.7" --with-openssl-dir=/opt/homebrew/opt/openssl@3 --enable-shared --with-libyaml-dir=/opt/homebrew/opt/libyaml --with-gmp-dir=/opt/homebrew/opt/gmp --with-ext=openssl,psych,+
-> make -j 10
-> make install
==> Installed ruby-3.4.7 to ~/.rbenv/versions/3.4.7

real 2m39.752s
user 4m41.813s
sys  1m35.644s
```

</details>

## Testimonials

"what the heckie that just installed a ruby version for me in .22 seconds???"
 &mdash; [c. ruiyi smallbird](https://bsky.app/profile/veryroundbird.house/post/3lxdwtsbwp22o)

"Holy wow that thing \_flies\_."
 &mdash; [Andrea Fomera](https://bsky.app/profile/afomera.dev/post/3m4wjfvqrhk2h)

## Requirements

- **Operating Systems**: macOS 14+, Linux glibc 2.35+
- **Architectures**: x86 on Intel, AMD, etc. and arm64 on Apple, Qualcomm, etc.
- **Ruby Versions**: All non-EOL'ed Ruby versions. Currently Ruby 3.2.x, 3.3.x, 3.4.1 and up.
- **Shells**: zsh, bash, fish, nushell. See [SHELL INTEGRATION](docs/SHELL_INTEGRATION.md) for more.

## Install

```bash
# Homebrew or Linuxbrew
brew install rv
```

Or, visit the [releases page](https://github.com/spinel-coop/rv/releases) for an installer shell script.

## Usage

You call `rv ruby run` and we'll install Ruby for you on the first run:

```bash
time rv ruby run 3.4.7 -- --version
```

> [!NOTE]
> We've prepended `time` to show how fast rv installs Ruby on the first run, it's not needed to use rv.

For automatic Ruby version selection, like `rbenv` or `chruby`, you can add a line of configuration to your shell. After this one-time setup, `rv` will automatically use `.ruby-version` or `.tool-versions` files to give you the requested Ruby. Set or update the version of Ruby used in a project by running `rv ruby pin VERSION`.

See [SHELL INTEGRATION](docs/SHELL_INTEGRATION.md) for instructions to configure zsh, bash, fish, and nushell.

## Brought to you by Spinel

[Spinel.coop](https://spinel.coop) makes engineering teams more effective with decades of lessons learned on the core teams of Rails, Hotwire, Bundler, and rbenv. Let us [multiply the team you already have](https://spinel.coop).

## Similar Work

See [Ruby Butler](https://github.com/RubyElders/ruby-butler) for similar next-level tooling ideas for Ruby.

We appreciate any tooling that improves Ruby and the lives of Ruby developers. We're all richer for people contributing their time & energy to make us all better. If you're reading this and you've contributed to Ruby in any way, thank you!

## Future Plans

All-in-one tooling for Ruby developers.

- Install & manage all Ruby versions.
- Manage gems, applications, CLI tools, and scripts, effortlessly.
- Handle everything Ruby provided by `rvm`, `rbenv`, `chruby`, `asdf`, `mise`, `ruby-build`, `ruby-install`, `bundler`, and `rubygems`, all at once.
- Install Ruby and all your gems in seconds, without having to compile anything ever again.
- Install `ruby-head` versions for easier development for Ruby core contributors.
- `mise` plugin, so you get fast no-fuss Ruby installs too.

### Future features

- Run any gem command instantly, like `rvx rails new`.
- Run any script, installing all needed gems, like `rv run script.rb`.
- Install gem CLIs with any needed rubies with `rv tool install`.
- Install precompiled Ruby versions in seconds with `rv ruby install`.
- Manage project gems with `rv install`, `rv add`, and `rv remove`.
- Create gems with `rv gem`, and publish them with `rv publish`.

See [PLANS.md](docs/PLANS.md) for more on our future plans.

## FAQ

**Does rv have a website?**

Yes! You're looking at it. You can reach this page with the URL [rv.dev](https://rv.dev).

**Why are you doing this?**

[@indirect](https://github.com/indirect) wrote [a blog post about his motivation to create `rv`](https://andre.arko.net/2025/08/25/rv-a-new-kind-of-ruby-management-tool/).

**How do you pronounce rv?**

"arr vee", the names of the two letters, spelled out.

## Contributing

Install dependencies on macOS or Ubuntu with `bin/setup`. Make changes, and then run the development binary with `bin/rv`, or install the development binary to your system with `bin/install`.

When you're done, make sure to run the tests with `bin/test`, and the linter with `bin/lint`. Then, send us a pull request! We love pull requests.

## Acknowledgements

`rv` is heavily inspired by [`uv`](https://github.com/astral-sh/uv), as you might guess from the name.

It also takes inspiration, features, and approaches from [Bundler](https://bundler.io), [Frum](https://github.com/TaKO8Ki/frum), [Homebrew](https://brew.sh), [npm](https://npmjs.com), [Cargo](https://github.com/rust-lang/cargo), and [Orogene](https://github.com/orogene/orogene).

We'd also like to thank everyone who has ever worked on a package manager in the past for helping get all of us to where we are today.

## License

`rv` is licensed under either [Apache-2.0](/LICENSE-APACHE) or [MIT](/LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion by you, as defined in the Apache-2.0 license, shall be dually licensed as above, without any additional terms or conditions.

# `rv`, the Ruby swiss army knife

Welcome to `rv`, a new Ruby version manager with high ambitions.

Today, you can install Ruby in one second flat.

Eventually, you'll be able to manage all your Rubies, tools, gems, and packages, faster than you would believe is possible.

## Testimonials

"what the heckie that just installed a ruby version for me in .22 seconds???"
 &mdash; <a href="https://bsky.app/profile/veryroundbird.house/post/3lxdwtsbwp22o">c. ruiyi smallbird</a>

## Requirements

- **Operating Systems**: macOS 14 and up, Ubuntu 24.04 and up
- **Ruby Versions**: Ruby 3.4.1 and up
- **Shells**: zsh, bash, fish

## Quickstart

```bash
brew install rv
# zsh
echo 'eval "$(rv shell init zsh)"' >> ~/.zshrc
eval "$(rv shell init zsh)"
# bash
echo 'eval "$(rv shell init bash)"' >> ~/.bashrc
eval "$(rv shell init bash)"
# fish
echo 'rv shell init fish | source' >> ~/.config/fish/config.fish
rv shell init fish | source
```

## Usage

```bash
echo "3.4.5" > .ruby-version
time rv ruby install 3.4.5
```

## Brought to you by Spinel

[Spinel.coop](https://spinel.coop) is a collective of Ruby open source maintainers building next-generation developer tooling, like `rv`, and offering [flat-rate, unlimited access to maintainers](https://spinel.coop/retainers) who come from the core teams of Rails, Hotwire, Bundler, RubyGems, rbenv, and more.

## Future plans

All-in-one tooling for Ruby developers.

- Manage Ruby versions, gems, applications, tools, and scripts, effortlessly.
- Get rid of `rvm`, `rbenv`, `chruby`, `asdf`, `mise`, `ruby-build`, `ruby-install`, `bundler`, and `rubygems`, all at once.
- Install Ruby and all your gems in seconds, without having to compile anything ever again.

### Future features

- Run any gem command instantly, like `rvx rails new`.
- Run any script, installing all needed gems, like `rv run script.rb`.
- Install gem CLIs with any needed rubies with `rv tool install`.
- Install precompiled Ruby versions in seconds with `rv ruby install`.
- Manage project gems with `rv install`, `rv add`, and `rv remove`.
- Create gems with `rv gem`, and publish them with `rv publish`.

See [PLANS.md](docs/PLANS.md) for more on our future plans.

## FAQ

**Why are you doing this?**

[@indirect](https://github.com/indirect) wrote [a blog post about his motivation to create `rv`](https://andre.arko.net/2025/08/25/rv-a-new-kind-of-ruby-management-tool/).

**How do you pronounce rv?**

"arr vee", the names of the two letters, spelled out.

## Contributing

Install dependencies on macOS or Ubuntu with `bin/setup`. Make changes, and then run the development binary with `bin/rv`, or install the development binary to your system with `bin/install`.

When you're done, make sure to run the tests with `bin/test`, and the linter with `bin/lint`. Then, send us a pull request! We love pull requests.

## Acknowledgements

`rv` is (as you might guess from the name) heavily inspired by [`uv`](https://github.com/astral-sh/uv).

It also takes inspiration, features, and approaches from [Bundler](https://bundler.io), [Frum](https://github.com/TaKO8Ki/frum), [Homebrew](https://brew.sh), [npm](https://npmjs.com), [Cargo](https://github.com/rust-lang/cargo), and [Orogene](https://github.com/orogene/orogene).

We'd also like to thank everyone who has ever worked on a package manager in the past for helping get all of us to where we are today.

## License

`rv` is licensed under either [Apache-2.0](/LICENSE-APACHE) or [MIT](/LICENSE-MIT), at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion by you, as defined in the Apache-2.0 license, shall be dually licensed as above, without any additional terms or conditions.

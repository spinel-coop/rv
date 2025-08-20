# `rv`, the Ruby swiss army knife

Welcome to `rv`, a new Ruby version manager with high ambitions.

Today, you can install Ruby in one second flat.

Eventually, you'll be able to manage all your Rubies, tools, gems, and packages, faster than you would believe is possible.

## Requirements

- **Operating Systems**: macOS 14 and up, Ubuntu 24.04 and up
- **Ruby Versions**: Ruby 3.4.1 and up
- **Shells**: zsh

## Quickstart

```bash
brew install spinel-coop/tap/rv
echo 'eval "$(rv shell init zsh)"' >> ~/.zshrc
eval "$(rv shell init zsh)"
```

## Usage

```bash
echo "3.4.5" > .ruby-version
time rv ruby install 3.4.5
```

## Brought to you by Spinel.coop

[Spinel.coop](https://spinel.coop) is a collective of Ruby open source maintainers  building next-generation developer tooling, like `rv`, and offering [flat-rate, unlimited access to experts from the core teams of Rails, Hotwire, Bundler, RubyGems, and more](https://spinel.coop/retainers).

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

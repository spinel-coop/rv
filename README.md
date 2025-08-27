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

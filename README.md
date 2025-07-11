# `rv`, the Ruby swiss army knife

All-in-one tooling for Ruby developers.

- Manage Ruby versions, gems, applications, tools, and scripts, effortlessly.
- Get rid of `rvm`, `rbenv`, `chruby`, `asdf`, `mise`, `ruby-build`, `ruby-install`, `bundler`, and `rubygems`, all at once.
- Install Ruby and all your gems in seconds, without having to compile anything ever again.

## Overview

- Run any gem command instantly, like `rvx rails new`.
- Run any script, installing all needed gems, like `rv run script.rb`.
- Install gem CLIs with any needed rubies with `rv tool install`.
- Install precompiled Ruby versions in seconds with `rv ruby install`.
- Manage project gems with `rv install`, `rv add`, and `rv remove`.
- Create gems with `rv gem`, and publish them with `rv publish`.

## Quickstart

```bash
curl spinel.dev/rv/install.sh | bash
rvx rails new my-app
```

## Commands

### Ruby versions

- rv ruby list
- rv ruby install
- rv ruby uninstall
- rv ruby pin

### Gem CLI tools

- rv tool install
- rv tool uninstall
- rv tool run / rvx

### Ruby scripts

- rv run my-script.rb
- rv add --script
- rv remove --script

### Gem development

- rv gem
- rv build
- rv publish

### Project dependencies

- rv init
- rv install
- rv add
- rv remove
- rv upgrade
- rv list
- rv tree

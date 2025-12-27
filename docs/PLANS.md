# `rv` plans

All-in-one tooling for Ruby developers.

- Manage Ruby versions, gems, applications, tools, and scripts, effortlessly.
- Handle everything Ruby provided by `rvm`, `rbenv`, `chruby`, `asdf`, `mise`, `ruby-build`, `ruby-install`, `bundler`, and `rubygems`, all at once.
- Install Ruby and all your gems in seconds, without having to compile anything ever again.

## features

- Run any gem command instantly, like `rvx rails new`.
- Run any script, installing all needed gems, like `rv run script.rb`.
- Install gem CLIs with any needed rubies with `rv tool install`.
- Install precompiled Ruby versions in seconds with `rv ruby install`.
- Manage project gems with `rv install`, `rv add`, and `rv remove`.
- Create gems with `rv gem`, and publish them with `rv publish`.

## similar tools

rv combines several functions that have previously been separate tools:

- ruby version manager (like `rvm`, `rbenv`, `chruby`)
- ruby version installer (like `ruby-build`, `ruby-install`)
- gem installer (like `rubygems`)
- dependency installer (like `bundler`)
- project runner (like `npm`, `make`, `rake`)
- package dev & publishing (like `rubygems` & `bundler`)
- running packages (like `gemx` or `npx`)
- installing tools (like `uv tool`, ruby and node lack this today)

## commands

### Ruby versions

- [x] `rv ruby list`
- [x] `rv ruby pin`
- [x] `rv ruby dir`
- [ ] `rv ruby install`
  - [x] `rv ruby install 3.4.x`
  - [x] `rv ruby install 3.3.x`
  - [x] `rv ruby install 3.2.x`
  - [ ] `rv ruby install jruby 10`
  - [ ] `rv ruby install truffleruby 24`
  - [ ] `rv ruby install truffleruby+graalvm 24`
  - [ ] `rv ruby install mruby 3.3`
- [x] `rv ruby uninstall`

### Gem CLI tools

- [ ] `rvx` / `rv exec` / `rv tool run`
- [ ] `rv tool install`
- [ ] `rv tool uninstall`

### Ruby scripts

- [ ] `rvr` / `rv run`
- [ ] `rv add --script`
- [ ] `rv remove --script`

### Projects

- [ ] `rv install`
- [ ] `rv run`
- [ ] `rv list`
- [ ] `rv init`
- [ ] `rv upgrade`
- [ ] `rv add`
- [ ] `rv remove`
- [ ] `rv tree`

### Gems

- [ ] `rv gem`
- [ ] `rv build`
- [ ] `rv publish`

### Shell integration

- [x] `rv shell zsh`
- [x] `rv shell bash`
- [x] `rv shell fish`
- [x] `rv shell nushell`
- [ ] `rv shell powershell`

#### Shell integration internal commands

- [x] `rv shell init`
- [x] `rv shell env`
- [x] `rv shell completions`


## configuration

rv can be configured globally, per user, and per project, as well as via ENV. configuration files use the [KDL](https://kdl.dev) document format.

settings we know we want to support include:

- ruby location(s)
- ruby installation location
- default ruby version

## projects

Projects are libraries, or packages, or applications. The required file indicating a project root is a `Gemfile`, but a project with a `Gemfile` might still be inside a workspace that aggregates together several projects.

A project root may be indicated by a `Gemfile`, an `rbproject.kdl` config file, a `.git` directory, a `.jj` directory, or other files to be added in the future.

Most `rv` commands (like `add,` `remove,` `install,` and `lock`) are scoped to a project and that project's dependencies. Some commands also interact with the user or global state, like `ruby install`, `tool install`, etc.

## project subtypes

Projects can be one or more of:

- application (like a Rails or Hanami app)
- gem (published library)
- library (structured like a gem but not published)

We may want to provide explicit support for libraries vs gems, although today Bundler treats them as the same.

Applications typically have their own framework generators, so it's unlikely we will need to build any application-specific functionality. On the other hand, gems and libraries are typically generated using `bundle gem` so we will likely want to support generation for that. On the third hand, we may want to offer an extremely fast templating and generator framework that applications could use instead of Thor.

## workspaces

Workspaces are a group of projects that all have their own dependencies, but are resolved and locked together as a group to ensure that one set of gems will work across all of the packages.

Supporting workspaces means commands like `rv add` are scoped to a single project (and the closest `Gemfile`), while resolving dependencies for `rv install` or `rv lock` should be scoped to the parent workspace (if one exists), the parent Gemfile, and all projects inside the parent Gemfile.

## ruby version file

A ruby version file (named `.ruby-version`) indicates the desired/locked Ruby version for a specific project or workspace. Any project intended to be usable by itself, outside of a workspace context, should declare a Ruby version.

The ruby version can also be provided by a [`.tool-versions` file](https://asdf-vm.com/manage/configuration.html#tool-versions), shared by tools like asdf and mise.

Finally, the ruby version can also be provided by `rbproject.kdl`, although tools other than `rv` may not be able to read that.

## ruby locations

By default, we look for rubies in `$XDG_DATA_HOME/rv/rubies`, `~/.data/rv/rubies`, `~/.rubies`, `/opt/rubies`, `/opt/homebrew/Cellar/ruby/`, `/usr/local/rubies`, and `/usr/local/Cellar/ruby`.

Sincew we respect `$XDG_DATA_HOME`, we install rubies into `~/.local/share/rv/rubies` on macOS and `~/.data/rv/rubies` on Linux by default.

## `run` vs `exec`

While Bundler invented the idea of `bundle exec` to run commands in the context of a bundle, both node and python have settled on different terminology: `run NAME` is for running commands inside a bundle, while `exec NAME` is for running a package that might not even be installed yet, whether inside an application or not, without adding the package to the current bundle.

## shortcut binaries

In `uv`, the `run` and `exec` subcommands are both so common that they have their own dedicated binaries: `uvr` and `uvx`. This mirrors the `npm` shortcut for `npm exec`, which is named `npx`. That implies we should provide shortcut binaries named `rvr` and `rvx`.

## what are "tools"?

Tools are executables provided by a package, installed in a fully isolated environment with their own Ruby version and gemset, and run directly via a command placed directly on the PATH. For example, you could run `rv tool install gist` to end up with a `gist` command that works correctly whether you are inside or outside a Gemfile directory, inside or outside `bundle exec`, or even regardless of whether you have Ruby installed at all when you first run `tool install`.

This is roughly the equivalent of `cargo install` or `go install`, but taking care of the additional concerns raised by needing an interpreter. `uv tool install` handles all of this extremely well for Python, but Ruby has never had a tool that does this.

## subcommands

Notes about the functionality and implementation of each command.

### run

The `run` command executes commands and files provided by the current project or filesystem. Contrast to `exec`, below, which executes commands provided by installing gems. There are several sources of commands for `run`: 1) the $PATH, 2) your project, 3) a file

1. Anything in your `$PATH` can be run by `rv run`, including `ruby`! In practice, this sets up a Ruby environment and then runs your command. For example `rv run --ruby 3.4.2 -- ruby` will run ruby 3.4.2 regardless of which ruby version is currently activated by the environment and the current project. Similarly, if you run `rv run bash` and then try to run `ruby`, you will get the Ruby version configured by rv.

2. Your project can provide commands for `run` in two ways: 1) by defining named commands in the `gem.kdl` configuration file, like `npm run` with `package.json`, or 2) declaring your project provides binaries, like the `gist` gem provides the `gist` command. If you are working on the `gist` gem, you can `rv run gist` to run the current project's command.

3. You can write a Ruby script and then run it with `rv run script.rb`. Scripts can optionally contain their own required ruby versions and rubygems dependencies, as a magic comment with the same structure as `gem.kdl`. If the script has configuration comments setting a required ruby version or depending on gems, rv will install that ruby version and those gems and then run thes cript. If the script does not declare any Ruby or gem dependencies, rv will simply ensure a Ruby is installed and use it to run the script.

### exec

The `exec` command runs commands provided by gems. Contrast with `run`, above, which executes commands provided by the current project or filesystem.

Just like `npm exec`, `uv exec`, and `gem exec`, `rv exec` will: find a package with the given name, install it, and run the executable inside that package with the same name. For example, `rv exec rails new .` will install ruby if needed, install rails if needed, and then run `rails new .`.

Similar to npm and uv, `rv exec rails@latest new .` will check for the newest version of Rails and make sure that is what gets run. Without the `@latest` included at the end of the package name, `exec` will prioritize speed and run an already-installed rails if it exists.

### ruby

The `ruby` subcommand manages ruby versions, using subcommands `install`, `uninstall`, `pin`, and `find`.

#### [install](/docs/rv/ruby/install.md)

The install command downloads a precompiled ruby for the current architecture and operating system, installing it into the rubies directory (which defaults to `~/.data/rv/rubies`).

#### [pin](/docs/rv/ruby/pin.md)

Pin with no arguments reports the project's currently chosen version of Ruby.

Pin with a version argument tries to set that version for the current project, validating the version, installing the version if needed, and then writing the version into the current project's `.ruby-version` file.

#### find

The `ruby find` subcommand returns the full path to the currently chosen Ruby interpreter. If passed an argument, it interprets that argument as a version request and prints the full path to a Ruby interpreter that satisfies the version request.

### shell

The `shell` subcommand handles integration with the user's shell, including automatic ruby version switching and completions for rv commands.

#### init

The `init` command prints out shellscript that sets up automatic version switching, and is intended to be used like `eval "$(rv shell init zsh)"` to set up zsh to then automatically run `eval "$(rv shell env)"` every time the user changes directories, which provides automatic version switching.

#### env

The `env` command prints out the env vars that need to be set for the currently-desired ruby version, like `RUBY_VERSION` and `PATH`. The output is expected to be `eval`ed by the shell to change the installation that will run as `ruby`.

#### completions

The `completions` command prints out shell-specific output that can be `eval`ed to set up tab-completion for subcommands and arguments to commands.

### tool

The tool subcommand manages binaries available on the PATH, ensuring that a usable Ruby is installed, the gem and all of its dependencies are installed, and a binary is created and put somewhere in the PATH. The binary needs to ignore the currently chosen ruby version, the current bundle environment, and anything else necessary to ensure that when it is invoked it will run completely independently.

## open questions

1. Should we build support for an `rbproject.kdl` or similar file to configure projects? It could potentially replace `Gemfile`, `.gemspec`, `.ruby-versions`, `.bundle/config`, `Rakefile`, `.rubocop.yaml`, and any other dependency, package, linter, or script configurations. It would be nice to end the reign of the Filefile, and it would provide a place for arbitrary machine-formatted data that tooling could use. It would also provide a location to configure future multi-project workspaces.

# Shell integration

rv integrates with [zsh](#zsh), [bash](#bash), [fish](#fish), and [nushell](#nushell).

Using the configuration below, or something equivalent, will set up `rv` to automatically read any `.ruby-version` or `.tool-versions` file right before every shell command. If necessary, the shell integration will change your PATH, GEM_HOME, and other env vars as needed to ensure that the `ruby` command will run the expected version of ruby.

## zsh

```zsh
echo 'eval "$(rv shell init zsh)"' >> ~/.zshrc
echo 'eval "$(rv shell completions zsh)"' >> ~/.zshrc
```

## bash

```bash
echo 'eval "$(rv shell init bash)"' >> ~/.bashrc
echo 'eval "$(rv shell completions bash)"' >> ~/.bashrc
```

## fish

```fish
# fish
echo 'rv shell init fish | source' >> ~/.config/fish/config.fish
echo 'rv shell completions fish | source' >> ~/.config/fish/config.fish
```

## nushell

```nushell
echo 'mkdir ($nu.data-dir | path join "vendor/autoload")
rv shell init nu | save -f ($nu.data-dir | path join "vendor/autoload/rv.nu")
rv shell completions nu | save --append ($nu.data-dir | path join "vendor/autoload/rv.nu")' | save --append $nu.config-path
```

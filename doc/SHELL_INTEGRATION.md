# Shell integration

rv integrates with [zsh](#zsh), [bash](#bash), [fish](#fish), and [nushell](#nushell).

## zsh

```zsh
echo 'eval "$(rv shell init zsh)"
eval $(rv shell completion zsh)"' >> ~/.zshrc
```

## bash

```bash
echo 'eval "$(rv shell init bash)"
eval "$(rv shell completion bash)"' >> ~/.bashrc
```

## fish

```fish
# fish
echo 'rv shell init fish | source
rv shell completion fish | source' >> ~/.config/fish/config.fish
```

## nushell

```nushell
echo 'mkdir ($nu.data-dir | path join "vendor/autoload")
rv shell init nu | save -f ($nu.data-dir | path join "vendor/autoload/rv.nu")
rv shell completions nu | save --append ($nu.data-dir | path join "vendor/autoload/rv.nu")' | save --append $nu.config-path
```

use crate::config::Config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = miette::Result<T, Error>;

pub fn init(config: &Config) -> Result<()> {
    print!(
        concat!(
            "autoload -U add-zsh-hook\n",
            "_rv_autoload_hook () {{\n",
            "    eval \"$({} shell env)\"\n",
            "}}\n",
            "add-zsh-hook chpwd _rv_autoload_hook\n",
            "_rv_autoload_hook\n",
        ),
        config.current_exe
    );
    Ok(())
}

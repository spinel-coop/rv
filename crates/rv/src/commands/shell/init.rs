#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = miette::Result<T, Error>;

pub fn init() -> Result<()> {
    print!(
        concat!(
            "autoload -U add-zsh-hook\n",
            "_rv_autoload_hook () {{\n",
            "    eval \"$({} shell env)\"\n",
            "}}\n",
            "add-zsh-hook chpwd _rv_autoload_hook\n",
            "_rv_autoload_hook\n",
        ),
        std::env::current_exe()?.to_str().unwrap()
    );
    Ok(())
}

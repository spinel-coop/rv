use anstream::stream::IsTerminal;

#[tokio::main]
async fn main() {
    if let Err(err) = rv_core::run().await {
        let is_tty = std::io::stderr().is_terminal();
        if is_tty {
            eprintln!("{:?}", miette::Report::new(err));
        } else {
            eprintln!("Error: {:?}", err);
        }
        std::process::exit(1);
    }
}

use clap::Parser;

fn main() {
    let cli = tl::cli::Cli::parse();
    match tl::cli::run(cli) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("tl: {e}");
            std::process::exit(1);
        }
    }
}

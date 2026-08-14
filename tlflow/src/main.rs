use clap::Parser;

fn main() {
    let cli = throughline::cli::Cli::parse();
    match throughline::cli::run(cli) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("tlflow: {e}");
            std::process::exit(1);
        }
    }
}

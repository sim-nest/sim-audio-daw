#![forbid(unsafe_code)]

mod index_check;
mod simdoc;
mod workspace_coverage;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("index-check") => index_check::run(args),
        Some("simdoc") => simdoc::run(args),
        Some("workspace-coverage") => workspace_coverage::run(args),
        _ => Err(format!(
            "usage: {} <simdoc|index-check|workspace-coverage> [--check]",
            args.first().map(String::as_str).unwrap_or("xtask")
        )),
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

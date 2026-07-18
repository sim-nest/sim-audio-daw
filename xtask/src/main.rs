#![forbid(unsafe_code)]

mod simdoc;
mod workspace_coverage;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("simdoc") => simdoc::run(args),
        Some("workspace-coverage") => workspace_coverage::run(args),
        _ => Err(format!(
            "usage: {} <simdoc|workspace-coverage> [--check]",
            args.first().map(String::as_str).unwrap_or("xtask")
        )),
    };
    if let Err(err) = result {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

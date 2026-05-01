use std::fs;
use std::path::PathBuf;

use clap::CommandFactory;
use vimdoc_language_server::cli::Cli;

fn main() -> std::io::Result<()> {
    let out_dir = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("man"), PathBuf::from);

    fs::create_dir_all(&out_dir)?;
    clap_mangen::generate_to(Cli::command(), &out_dir)?;

    for entry in fs::read_dir(&out_dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "1") {
            let text = fs::read_to_string(&path)?;
            let normalized = text
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            if text != normalized {
                fs::write(path, normalized)?;
            }
        }
    }

    Ok(())
}

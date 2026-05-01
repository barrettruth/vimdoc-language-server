use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

use crate::formatter::ReflowMode;

#[derive(Clone, Copy, ValueEnum)]
pub enum CliReflowMode {
    Always,
    #[value(name = "only-if-too-long")]
    OnlyIfTooLong,
    Never,
}

impl From<CliReflowMode> for ReflowMode {
    fn from(m: CliReflowMode) -> Self {
        match m {
            CliReflowMode::Always => ReflowMode::Always,
            CliReflowMode::OnlyIfTooLong => ReflowMode::OnlyIfTooLong,
            CliReflowMode::Never => ReflowMode::Never,
        }
    }
}

#[derive(Parser)]
#[command(
    version,
    propagate_version = true,
    about = "Language server for Vim help files",
    long_about = "vimdoc-language-server provides Language Server Protocol support and standalone CLI tools for Vim help files. It can format vimdoc text, report duplicate tags and unresolved tag links, and run as an editor language server over stdio."
)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    #[arg(
        long,
        short = 'v',
        action = ArgAction::Count,
        global = true,
        help = "Increase logging verbosity; repeat for more detail"
    )]
    pub verbose: u8,

    #[arg(
        long,
        value_name = "PATH",
        global = true,
        help = "Write logs to PATH instead of stderr"
    )]
    pub log_file: Option<PathBuf>,

    #[arg(
        long,
        default_value_t = 78,
        value_name = "N",
        global = true,
        help = "Target line width for vimdoc formatting"
    )]
    pub line_width: usize,

    #[arg(
        long,
        value_name = "PATH",
        global = true,
        help = "Load additional Vim tags from PATH"
    )]
    pub tag_path: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Option<CliCommand>,

    #[arg(
        long,
        overrides_with = "no_runtime_tags",
        global = true,
        help = "Load tags from the discovered Vim runtime"
    )]
    pub runtime_tags: bool,

    #[arg(
        long,
        overrides_with = "runtime_tags",
        global = true,
        help = "Do not load tags from the discovered Vim runtime"
    )]
    pub no_runtime_tags: bool,

    #[arg(
        long,
        overrides_with = "no_formatting",
        help = "Enable editor formatting support"
    )]
    pub formatting: bool,

    #[arg(
        long,
        overrides_with = "formatting",
        help = "Disable editor formatting support"
    )]
    pub no_formatting: bool,

    #[arg(
        long,
        value_enum,
        default_value = "always",
        help = "Choose when prose paragraphs are reflowed"
    )]
    pub reflow: CliReflowMode,

    #[arg(long, help = "Normalize spacing between sentences while formatting")]
    pub normalize_spacing: bool,

    #[arg(
        long,
        overrides_with = "no_diagnostics",
        help = "Enable editor diagnostics"
    )]
    pub diagnostics: bool,

    #[arg(
        long,
        overrides_with = "diagnostics",
        help = "Disable editor diagnostics"
    )]
    pub no_diagnostics: bool,

    #[arg(long, overrides_with = "no_hover", help = "Enable hover responses")]
    pub hover: bool,

    #[arg(long, overrides_with = "hover", help = "Disable hover responses")]
    pub no_hover: bool,

    #[arg(
        long,
        overrides_with = "no_color",
        global = true,
        help = "Always use colored CLI output"
    )]
    pub color: bool,

    #[arg(
        long,
        overrides_with = "color",
        global = true,
        help = "Never use colored CLI output"
    )]
    pub no_color: bool,

    #[arg(long, help = "Print the editor initialization options JSON schema")]
    pub print_config_schema: bool,
}

#[derive(Subcommand)]
pub enum CliCommand {
    #[command(about = "Check vimdoc files for diagnostics")]
    Check(CheckArgs),
    #[command(about = "Format vimdoc files")]
    Format(FormatArgs),
}

#[derive(Args)]
pub struct CheckArgs {
    #[arg(help = "Directory containing vimdoc files")]
    pub path: PathBuf,

    #[arg(long, value_name = "CODE", help = "Ignore a diagnostic code")]
    pub ignore: Vec<String>,
}

#[derive(Args)]
pub struct FormatArgs {
    #[arg(
        value_name = "PATH",
        required = true,
        num_args = 1..,
        help = "Vimdoc file or directory to format"
    )]
    pub paths: Vec<PathBuf>,

    #[arg(long, help = "Report files that would change without writing them")]
    pub check: bool,
}

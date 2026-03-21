use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use lsp_server::Connection;
use lsp_types::{
    CompletionOptions, DiagnosticOptions, DiagnosticServerCapabilities, DiagnosticSeverity,
    InitializeParams, OneOf, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
};
use tracing_subscriber::EnvFilter;

use vimdoc_language_server::{
    diagnostics::{self, DiagnosticLevel},
    formatter::ReflowMode,
    server::{self, Config, InitOptions},
    tags::{self, TagIndex},
};

#[derive(Clone, Copy, ValueEnum)]
enum CliReflowMode {
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
#[command(version, about = "Language server for vim help files")]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    #[arg(long, short = 'v', action = ArgAction::Count, global = true)]
    verbose: u8,

    #[arg(long, value_name = "PATH", global = true)]
    log_file: Option<PathBuf>,

    #[arg(long, default_value_t = 78, value_name = "N", global = true)]
    line_width: usize,

    #[arg(long, value_name = "PATH", global = true)]
    tag_path: Vec<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,

    #[arg(long, overrides_with = "no_runtime_tags", global = true)]
    runtime_tags: bool,

    #[arg(long, overrides_with = "runtime_tags", global = true)]
    no_runtime_tags: bool,

    #[arg(long, overrides_with = "no_formatting")]
    formatting: bool,

    #[arg(long, overrides_with = "formatting")]
    no_formatting: bool,

    #[arg(long, value_enum, default_value = "always")]
    reflow: CliReflowMode,

    #[arg(long)]
    normalize_spacing: bool,

    #[arg(long, overrides_with = "no_diagnostics")]
    diagnostics: bool,

    #[arg(long, overrides_with = "diagnostics")]
    no_diagnostics: bool,

    #[arg(long, overrides_with = "no_hover")]
    hover: bool,

    #[arg(long, overrides_with = "hover")]
    no_hover: bool,

    #[arg(long, overrides_with = "no_color", global = true)]
    color: bool,

    #[arg(long, overrides_with = "color", global = true)]
    no_color: bool,

    #[arg(long)]
    print_config_schema: bool,
}

#[derive(Subcommand)]
enum Command {
    Check(CheckArgs),
}

#[derive(Args)]
struct CheckArgs {
    path: PathBuf,
    #[arg(long, value_name = "CODE")]
    ignore: Vec<String>,
}

fn server_capabilities(cli: &Cli) -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        document_formatting_provider: if cli.no_formatting {
            None
        } else {
            Some(OneOf::Left(true))
        },
        document_range_formatting_provider: if cli.no_formatting {
            None
        } else {
            Some(OneOf::Left(true))
        },
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(lsp_types::RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: lsp_types::WorkDoneProgressOptions::default(),
        })),
        hover_provider: if cli.no_hover {
            None
        } else {
            Some(lsp_types::HoverProviderCapability::Simple(true))
        },
        document_highlight_provider: Some(OneOf::Left(true)),
        folding_range_provider: Some(lsp_types::FoldingRangeProviderCapability::Simple(true)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec!["|".to_string()]),
            ..CompletionOptions::default()
        }),
        document_link_provider: Some(lsp_types::DocumentLinkOptions {
            resolve_provider: Some(false),
            work_done_progress_options: lsp_types::WorkDoneProgressOptions::default(),
        }),
        code_action_provider: Some(lsp_types::CodeActionProviderCapability::Simple(true)),
        diagnostic_provider: if cli.no_diagnostics {
            None
        } else {
            Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
                identifier: Some("vimdoc".into()),
                inter_file_dependencies: true,
                workspace_diagnostics: true,
                work_done_progress_options: lsp_types::WorkDoneProgressOptions::default(),
            }))
        },
        ..Default::default()
    }
}

fn init_tracing(cli: &Cli) -> Result<()> {
    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    if let Some(ref log_path) = cli.log_file {
        let file = std::fs::File::create(log_path)?;
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    }
    Ok(())
}

fn resolve_color(cli: &Cli) -> bool {
    use std::io::IsTerminal;
    if cli.no_color || std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if cli.color {
        return true;
    }
    if std::env::var_os("CLICOLOR_FORCE").is_some() {
        return true;
    }
    if std::env::var("CLICOLOR").as_deref() == Ok("0") {
        return false;
    }
    if std::env::var("TERM").as_deref() == Ok("dumb") {
        return false;
    }
    std::io::stdout().is_terminal()
}

fn colorize(text: &str, codes: &str, use_color: bool) -> String {
    if use_color {
        format!("\x1b[{codes}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn run_check(args: &CheckArgs, cli: &Cli) -> Result<()> {
    let mut tag_index = TagIndex::new();
    tag_index.scan_directory(&args.path)?;

    for tp in &cli.tag_path {
        server::load_tag_path(&mut tag_index, tp);
    }

    if !cli.no_runtime_tags {
        if let Some(runtime_path) = tags::discover_vimruntime() {
            tag_index.load_runtime_tags(&runtime_path)?;
        }
    }

    let mut levels: HashMap<String, DiagnosticLevel> = HashMap::new();
    for code in &args.ignore {
        levels.insert(code.clone(), DiagnosticLevel::Off);
    }

    let use_color = resolve_color(cli);
    let mut total = 0u32;
    let mut blocking = 0u32;
    let mut files_with_diags = 0u32;
    let dir_abs = std::fs::canonicalize(&args.path)?;

    for (uri, doc) in tag_index.workspace_docs() {
        let diags = diagnostics::compute(doc, &tag_index, uri, &levels);
        if diags.is_empty() {
            continue;
        }
        files_with_diags += 1;
        let display_path = server::uri_to_path(uri)
            .and_then(|p| p.strip_prefix(&dir_abs).ok().map(Path::to_path_buf))
            .map_or_else(|| uri.as_str().to_string(), |p| p.display().to_string());
        for d in &diags {
            let line = d.range.start.line + 1;
            let col = d.range.start.character + 1;
            let code = d.code.as_ref().map_or("warning".to_string(), |c| match c {
                lsp_types::NumberOrString::String(s) => s.clone(),
                lsp_types::NumberOrString::Number(n) => n.to_string(),
            });
            let code_color = match d.severity {
                Some(DiagnosticSeverity::ERROR) => "1;31",
                Some(DiagnosticSeverity::WARNING) => "1;33",
                Some(DiagnosticSeverity::INFORMATION) => "1;34",
                _ => "2",
            };
            let loc = format!("{display_path}:{line}:{col}");
            println!(
                "{}: {} {}",
                colorize(&loc, "1", use_color),
                colorize(&format!("[{code}]"), code_color, use_color),
                d.message,
            );
            if matches!(
                d.severity,
                Some(DiagnosticSeverity::ERROR | DiagnosticSeverity::WARNING)
            ) {
                blocking += 1;
            }
        }
        #[allow(clippy::cast_possible_truncation)]
        {
            total += diags.len() as u32;
        }
    }

    let summary = format!("{total} diagnostics in {files_with_diags} file(s)");
    println!(
        "{}",
        colorize(
            &summary,
            if blocking == 0 { "1;32" } else { "1;33" },
            use_color
        )
    );
    if blocking > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn print_config_schema() -> Result<()> {
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft-07/schema",
        "title": "vimdoc-language-server",
        "description": "initializationOptions for vimdoc-language-server",
        "type": "object",
        "properties": {
            "lineWidth": {
                "type": "integer",
                "minimum": 1,
                "default": 78,
                "description": "Target line width for formatting"
            },
            "formatting": {
                "type": "boolean",
                "default": true,
                "description": "Enable document formatting"
            },
            "reflow": {
                "type": "string",
                "enum": ["always", "only-if-too-long", "never"],
                "default": "always",
                "description": "Prose reflow mode"
            },
            "normalizeSpacing": {
                "type": "boolean",
                "default": false,
                "description": "Normalize spacing between sentences"
            },
            "diagnostics": {
                "type": "boolean",
                "default": true,
                "description": "Enable diagnostics"
            },
            "hover": {
                "type": "boolean",
                "default": true,
                "description": "Enable hover"
            },
            "runtimeTags": {
                "type": "boolean",
                "default": true,
                "description": "Load tags from $VIMRUNTIME/doc/tags"
            },
            "tagPaths": {
                "type": "array",
                "items": { "type": "string" },
                "default": [],
                "description": "Additional Vim tags file paths to load"
            },
            "diagnosticLevels": {
                "type": "object",
                "additionalProperties": {
                    "type": "string",
                    "enum": ["error", "warning", "information", "hint", "off"]
                },
                "default": {},
                "description": "Per-diagnostic severity overrides. Keys are diagnostic codes, values are levels."
            }
        },
        "additionalProperties": false
    });
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.print_config_schema {
        return print_config_schema();
    }

    if let Some(Command::Check(ref args)) = cli.command {
        return run_check(args, &cli);
    }

    init_tracing(&cli)?;

    let (connection, io_threads) = Connection::stdio();

    let (init_id, init_params_value) = connection.initialize_start()?;
    let init_result = serde_json::json!({
        "capabilities": server_capabilities(&cli),
        "serverInfo": {
            "name": "vimdoc-language-server",
            "version": env!("CARGO_PKG_VERSION"),
        }
    });
    connection.initialize_finish(init_id, init_result)?;
    let init_params: InitializeParams = serde_json::from_value(init_params_value)?;

    let init_opts: InitOptions = init_params
        .initialization_options
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let mut tag_paths = cli.tag_path.clone();
    tag_paths.extend(init_opts.tag_paths);

    let config = Config {
        line_width: init_opts.line_width.unwrap_or(cli.line_width),
        formatting: init_opts.formatting.unwrap_or(!cli.no_formatting),
        reflow: init_opts.reflow.unwrap_or_else(|| cli.reflow.into()),
        normalize_spacing: init_opts.normalize_spacing.unwrap_or(cli.normalize_spacing),
        diagnostics: init_opts.diagnostics.unwrap_or(!cli.no_diagnostics),
        hover: init_opts.hover.unwrap_or(!cli.no_hover),
        runtime_tags: init_opts.runtime_tags.unwrap_or(!cli.no_runtime_tags),
        tag_paths,
        diagnostic_levels: init_opts.diagnostic_levels,
    };

    let workspace_root = init_params
        .workspace_folders
        .as_ref()
        .and_then(|wf| wf.first())
        .and_then(|f| server::uri_to_path(&f.uri))
        .or_else(|| {
            #[allow(deprecated)]
            init_params.root_uri.as_ref().and_then(server::uri_to_path)
        });

    let mut tag_index = TagIndex::new();

    if let Some(ref root) = workspace_root {
        if let Err(e) = tag_index.scan_workspace(root) {
            tracing::warn!(error = %e, "failed to scan workspace");
        }
    }

    for tp in &config.tag_paths {
        server::load_tag_path(&mut tag_index, tp);
    }

    if config.runtime_tags {
        if let Some(runtime_path) = tags::discover_vimruntime() {
            if let Err(e) = tag_index.load_runtime_tags(&runtime_path) {
                tracing::warn!(error = %e, "failed to load runtime tags");
            }
        } else {
            tracing::warn!("could not discover $VIMRUNTIME, runtime tags not loaded");
        }
    }

    tracing::info!(
        line_width = config.line_width,
        formatting = config.formatting,
        reflow = ?config.reflow,
        normalize_spacing = config.normalize_spacing,
        diagnostics = config.diagnostics,
        hover = config.hover,
        "server initialized"
    );

    server::main_loop(&connection, &config, &mut tag_index)?;

    io_threads.join()?;
    Ok(())
}

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{ArgAction, Parser, ValueEnum};
use lsp_server::Connection;
use lsp_types::{
    CompletionOptions, DiagnosticOptions, DiagnosticServerCapabilities, InitializeParams, OneOf,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
};
use tracing_subscriber::EnvFilter;

use vimdoc_language_server::{
    diagnostics,
    formatter::ReflowMode,
    server::{self, Config, InitOptions},
    tags::TagIndex,
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
    #[arg(long, short = 'v', action = ArgAction::Count)]
    verbose: u8,

    #[arg(long, value_name = "PATH")]
    log_file: Option<PathBuf>,

    #[arg(long, default_value_t = 78, value_name = "N")]
    line_width: usize,

    #[arg(long)]
    no_formatting: bool,

    #[arg(long, value_enum, default_value = "always")]
    reflow: CliReflowMode,

    #[arg(long)]
    normalize_spacing: bool,

    #[arg(long)]
    no_diagnostics: bool,

    #[arg(long)]
    no_hover: bool,

    #[arg(long)]
    print_config_schema: bool,

    #[arg(long, value_name = "PATH")]
    tag_path: Vec<PathBuf>,

    #[arg(long)]
    no_runtime_tags: bool,

    #[arg(long, value_name = "PATH")]
    check: Option<PathBuf>,
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

fn load_pack_tags(tag_index: &mut TagIndex, runtime: &Path) {
    for subdir in &["pack/*/opt/*/doc/tags", "pack/*/start/*/doc/tags"] {
        let pattern = runtime.join(subdir);
        if let Some(s) = pattern.to_str() {
            for path in glob::glob(s).into_iter().flatten().flatten() {
                if let Err(e) = tag_index.load_tags_file(&path) {
                    tracing::warn!(path = %path.display(), error = %e, "failed to load pack tags");
                }
            }
        }
    }
}

fn run_check(dir: &Path, cli: &Cli) -> Result<()> {
    let mut tag_index = TagIndex::new();
    tag_index.scan_directory(dir)?;

    for tp in &cli.tag_path {
        server::load_tag_path(&mut tag_index, tp);
    }

    if !cli.no_runtime_tags {
        if let Ok(runtime) = std::env::var("VIMRUNTIME") {
            let runtime_path = Path::new(&runtime);
            let tags_file = runtime_path.join("doc/tags");
            if tags_file.exists() {
                let _ = tag_index.load_tags_file(&tags_file);
            }
            load_pack_tags(&mut tag_index, runtime_path);
        }
    }

    let mut total = 0u32;
    let mut files_with_diags = 0u32;
    let dir_abs = std::fs::canonicalize(dir)?;

    for (uri, doc) in tag_index.workspace_docs() {
        let diags = diagnostics::compute(doc, &tag_index, uri);
        if diags.is_empty() {
            continue;
        }
        files_with_diags += 1;
        let display_path = server::uri_to_path(uri)
            .and_then(|p| p.strip_prefix(&dir_abs).ok().map(Path::to_path_buf))
            .map_or_else(|| uri.as_str().to_string(), |p| p.display().to_string());
        for d in &diags {
            let line = d.range.start.line + 1;
            let code = d.code.as_ref().map_or("warning".to_string(), |c| match c {
                lsp_types::NumberOrString::String(s) => s.clone(),
                lsp_types::NumberOrString::Number(n) => n.to_string(),
            });
            eprintln!("{display_path}:{line}: [{code}] {}", d.message);
        }
        #[allow(clippy::cast_possible_truncation)]
        {
            total += diags.len() as u32;
        }
    }

    eprintln!("{total} warnings in {files_with_diags} file(s)");
    if total > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.print_config_schema {
        println!("{{}}");
        return Ok(());
    }

    if let Some(ref dir) = cli.check {
        return run_check(dir, &cli);
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
        if let Ok(runtime) = std::env::var("VIMRUNTIME") {
            let runtime_path = Path::new(&runtime);
            let tags_file = runtime_path.join("doc/tags");
            if tags_file.exists() {
                if let Err(e) = tag_index.load_tags_file(&tags_file) {
                    tracing::warn!(path = %tags_file.display(), error = %e, "failed to load runtime tags");
                }
            }
            load_pack_tags(&mut tag_index, runtime_path);
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

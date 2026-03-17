use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{ArgAction, Parser};
use lsp_server::Connection;
use lsp_types::{
    CompletionOptions, InitializeParams, OneOf, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind,
};
use tracing_subscriber::EnvFilter;

use vimdoc_language_server::{
    server::{self, Config, InitOptions},
    tags::TagIndex,
};

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
}

fn server_capabilities(cli: &Cli) -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        document_formatting_provider: if cli.no_formatting {
            None
        } else {
            Some(OneOf::Left(true))
        },
        document_symbol_provider: Some(OneOf::Left(true)),
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.print_config_schema {
        println!("{{}}");
        return Ok(());
    }

    init_tracing(&cli)?;

    let (connection, io_threads) = Connection::stdio();

    let server_caps = serde_json::to_value(server_capabilities(&cli))?;
    let init_params: InitializeParams =
        serde_json::from_value(connection.initialize(server_caps)?)?;

    let init_opts: InitOptions = init_params
        .initialization_options
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let mut tag_paths = cli.tag_path.clone();
    tag_paths.extend(init_opts.tag_paths);

    let config = Config {
        line_width: cli.line_width,
        formatting: !cli.no_formatting,
        diagnostics: !cli.no_diagnostics,
        hover: !cli.no_hover,
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
            let tags_file = Path::new(&runtime).join("doc/tags");
            if tags_file.exists() {
                if let Err(e) = tag_index.load_tags_file(&tags_file) {
                    tracing::warn!(path = %tags_file.display(), error = %e, "failed to load runtime tags");
                }
            }
        }
    }

    tracing::info!(
        line_width = config.line_width,
        formatting = config.formatting,
        diagnostics = config.diagnostics,
        hover = config.hover,
        "server initialized"
    );

    server::main_loop(&connection, &config, &mut tag_index)?;

    io_threads.join()?;
    Ok(())
}

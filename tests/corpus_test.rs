use std::path::PathBuf;

use lsp_types::Uri;
use vimdoc_language_server::{
    diagnostics,
    formatter::{FormatOptions, format_document},
    parser::Document,
    tags::TagIndex,
};

fn find_vimruntime() -> Option<PathBuf> {
    if let Ok(v) = std::env::var("VIMRUNTIME") {
        let p = PathBuf::from(&v);
        if p.exists() {
            return Some(p);
        }
    }
    let output = std::process::Command::new("which")
        .arg("nvim")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let nvim_str = std::str::from_utf8(&output.stdout).ok()?.trim().to_string();
    let nvim_path = PathBuf::from(&nvim_str);
    let resolved = std::fs::canonicalize(&nvim_path).ok()?;
    let prefix = resolved.parent()?.parent()?;
    let runtime = prefix.join("share/nvim/runtime");
    if runtime.exists() {
        Some(runtime)
    } else {
        None
    }
}

fn doc_txt_files(doc_dir: &std::path::Path) -> Vec<PathBuf> {
    std::fs::read_dir(doc_dir)
        .expect("read doc dir")
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect()
}

#[test]
fn vimruntime_tags_load_and_resolve() {
    let Some(vimruntime) = find_vimruntime() else {
        return;
    };
    let tags_file = vimruntime.join("doc/tags");
    if !tags_file.exists() {
        return;
    }

    let mut tag_index = TagIndex::default();
    tag_index
        .load_tags_file(&tags_file)
        .expect("load tags file");

    for tag in &["$VIMRUNTIME", "$VIM"] {
        assert!(
            tag_index.contains(tag),
            "expected tag {tag:?} in vimruntime tags"
        );
    }

    assert!(
        tag_index.resolve("$VIMRUNTIME").is_some(),
        "resolve $VIMRUNTIME should return a location"
    );
}

#[test]
fn formatter_idempotent_on_neovim_corpus() {
    let Some(vimruntime) = find_vimruntime() else {
        return;
    };
    let files = doc_txt_files(&vimruntime.join("doc"));
    assert!(!files.is_empty(), "no .txt files found in runtime/doc");

    for path in &files {
        let text = std::fs::read_to_string(path).expect("read file");
        let opts = FormatOptions::default();
        let once = format_document(&text, &opts);
        let twice = format_document(&once, &opts);
        assert_eq!(once, twice, "formatter not idempotent on {path:?}");
    }
}

#[test]
fn diagnostics_no_panic_on_neovim_corpus() {
    let Some(vimruntime) = find_vimruntime() else {
        return;
    };
    let doc_dir = vimruntime.join("doc");
    let tags_file = doc_dir.join("tags");

    let mut tag_index = TagIndex::default();
    if tags_file.exists() {
        let _ = tag_index.load_tags_file(&tags_file);
    }

    let files = doc_txt_files(&doc_dir);
    assert!(!files.is_empty(), "no .txt files found in runtime/doc");

    for path in &files {
        let text = std::fs::read_to_string(path).expect("read file");
        let uri_str = format!("file://{}", path.display());
        let Ok(uri) = uri_str.parse::<Uri>() else {
            continue;
        };
        let doc = Document::parse(&text);
        let _ = diagnostics::compute(&doc, &tag_index, &uri);
    }
}

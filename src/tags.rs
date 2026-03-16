use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use lsp_types::{Range, Uri};

use crate::parser::Document;

#[derive(Debug, Clone)]
pub struct TagEntry {
    pub uri: Uri,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct ExternalTag {
    pub file_path: PathBuf,
    pub search_pattern: String,
}

#[derive(Debug, Default)]
pub struct TagIndex {
    workspace: HashMap<String, Vec<TagEntry>>,
    external: HashMap<String, Vec<ExternalTag>>,
    external_cache: HashMap<PathBuf, Document>,
}

impl TagIndex {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_file(&mut self, uri: &Uri, doc: &Document) {
        self.remove_file(uri);
        for span in doc.tag_defs() {
            self.workspace
                .entry(span.name.clone())
                .or_default()
                .push(TagEntry {
                    uri: uri.clone(),
                    range: span.range,
                });
        }
    }

    pub fn remove_file(&mut self, uri: &Uri) {
        self.workspace.retain(|_, entries| {
            entries.retain(|e| &e.uri != uri);
            !entries.is_empty()
        });
    }

    pub fn resolve(&mut self, name: &str) -> Option<TagEntry> {
        if let Some(entries) = self.workspace.get(name) {
            return entries.first().cloned();
        }
        self.resolve_external(name)
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn scan_workspace(&mut self, root: &Path) -> Result<()> {
        let pattern = root.join("doc/**/*.txt");
        let pattern_str = pattern.to_str().ok_or_else(|| anyhow!("non-UTF-8 path"))?;
        for entry in glob::glob(pattern_str)? {
            let path = entry?;
            if let Ok(text) = fs::read_to_string(&path) {
                let uri = path_to_uri(&path)?;
                let doc = Document::parse(&text);
                self.update_file(&uri, &doc);
            }
        }
        Ok(())
    }

    #[allow(clippy::missing_errors_doc)]
    pub fn load_tags_file(&mut self, path: &Path) -> Result<()> {
        let entries = parse_tags_file(path)?;
        for (name, ext) in entries {
            self.external.entry(name).or_default().push(ext);
        }
        Ok(())
    }

    fn resolve_external(&mut self, name: &str) -> Option<TagEntry> {
        let externals = self.external.get(name)?;
        for ext in externals.clone() {
            let Some(doc) = self.parse_external_cached(&ext.file_path) else {
                continue;
            };
            let Some(def) = doc.tag_defs().find(|d| d.name == name) else {
                continue;
            };
            let Ok(uri) = path_to_uri(&ext.file_path) else {
                continue;
            };
            return Some(TagEntry {
                uri,
                range: def.range,
            });
        }
        None
    }

    fn parse_external_cached(&mut self, path: &PathBuf) -> Option<&Document> {
        if !self.external_cache.contains_key(path) {
            let text = fs::read_to_string(path).ok()?;
            let doc = Document::parse(&text);
            self.external_cache.insert(path.clone(), doc);
        }
        self.external_cache.get(path)
    }
}

#[allow(clippy::missing_errors_doc)]
pub fn parse_tags_file(path: &Path) -> Result<Vec<(String, ExternalTag)>> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let content = fs::read_to_string(path)?;
    let mut entries = Vec::new();

    for line in content.lines() {
        if line.starts_with('!') || line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let tag_name = match parts.next() {
            Some(n) if !n.is_empty() => n,
            _ => continue,
        };
        let file_name = match parts.next() {
            Some(f) if !f.is_empty() => f,
            _ => continue,
        };
        let search_pattern = parts.next().unwrap_or_default().to_string();
        entries.push((
            tag_name.to_string(),
            ExternalTag {
                file_path: dir.join(file_name),
                search_pattern,
            },
        ));
    }

    Ok(entries)
}

fn path_to_uri(path: &Path) -> Result<Uri> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let url_str = format!("file://{}", abs.display());
    url_str.parse::<Uri>().map_err(|e| anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_workspace(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        let doc_dir = dir.path().join("doc");
        fs::create_dir_all(&doc_dir).unwrap();
        for (name, content) in files {
            let path = doc_dir.join(name);
            let mut f = fs::File::create(path).unwrap();
            f.write_all(content.as_bytes()).unwrap();
        }
        dir
    }

    #[test]
    fn scan_workspace_finds_tags() {
        let dir = make_workspace(&[("foo.txt", "*foo-tag* some heading\n")]);
        let mut index = TagIndex::new();
        index.scan_workspace(dir.path()).unwrap();
        assert!(index.resolve("foo-tag").is_some());
    }

    #[test]
    fn update_and_remove_file() {
        let mut index = TagIndex::new();
        let uri: Uri = "file:///tmp/test.txt".parse().unwrap();
        let doc = Document::parse("*alpha* heading\n*beta* heading\n");
        index.update_file(&uri, &doc);
        assert!(index.resolve("alpha").is_some());
        assert!(index.resolve("beta").is_some());
        index.remove_file(&uri);
        assert!(index.resolve("alpha").is_none());
    }

    #[test]
    fn parse_tags_file_basic() {
        let dir = TempDir::new().unwrap();
        let tags_path = dir.path().join("tags");
        fs::write(
            &tags_path,
            "!_TAG_FILE_SORTED\t1\noptions\toptions.txt\t/*options*\n",
        )
        .unwrap();
        let entries = parse_tags_file(&tags_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "options");
        assert_eq!(entries[0].1.file_path, dir.path().join("options.txt"));
    }

    #[test]
    fn cross_file_resolve() {
        let dir = make_workspace(&[("a.txt", "*tag-a* heading\n"), ("b.txt", "see |tag-a|\n")]);
        let mut index = TagIndex::new();
        index.scan_workspace(dir.path()).unwrap();
        let entry = index.resolve("tag-a").unwrap();
        assert!(entry.uri.as_str().contains("a.txt"));
    }

    #[test]
    fn external_tag_resolve() {
        let dir = TempDir::new().unwrap();
        let doc_content = "*ext-tag* some heading\n";
        let doc_path = dir.path().join("ext.txt");
        fs::write(&doc_path, doc_content).unwrap();
        let tags_content = "ext-tag\text.txt\t/*ext-tag*\n";
        let tags_path = dir.path().join("tags");
        fs::write(&tags_path, tags_content).unwrap();

        let mut index = TagIndex::new();
        index.load_tags_file(&tags_path).unwrap();
        let entry = index.resolve("ext-tag").unwrap();
        assert!(entry.uri.as_str().contains("ext.txt"));
    }

    #[test]
    fn workspace_takes_priority_over_external() {
        let dir = TempDir::new().unwrap();
        let ext_path = dir.path().join("ext.txt");
        fs::write(&ext_path, "*shared* external heading\n").unwrap();
        let tags_path = dir.path().join("tags");
        fs::write(&tags_path, "shared\text.txt\t/*shared*\n").unwrap();

        let mut index = TagIndex::new();
        index.load_tags_file(&tags_path).unwrap();

        let ws_uri: Uri = "file:///workspace/doc/ws.txt".parse().unwrap();
        let ws_doc = Document::parse("*shared* workspace heading\n");
        index.update_file(&ws_uri, &ws_doc);

        let entry = index.resolve("shared").unwrap();
        assert!(entry.uri.as_str().contains("ws.txt"));
    }
}

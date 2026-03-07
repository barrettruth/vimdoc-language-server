use std::collections::HashMap;

use lsp_types::Uri;

use crate::parser::Document;

#[derive(Default)]
pub struct Store {
    docs: HashMap<Uri, (String, Document)>,
}

impl Store {
    pub fn open(&mut self, uri: Uri, text: String) {
        let doc = Document::parse(&text);
        self.docs.insert(uri, (text, doc));
    }

    pub fn change(&mut self, uri: &Uri, text: String) {
        let doc = Document::parse(&text);
        self.docs.insert(uri.clone(), (text, doc));
    }

    pub fn close(&mut self, uri: &Uri) {
        self.docs.remove(uri);
    }

    pub fn get(&self, uri: &Uri) -> Option<(&str, &Document)> {
        self.docs
            .get(uri)
            .map(|(t, d): &(String, Document)| (t.as_str(), d))
    }
}

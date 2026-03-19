use anyhow::Result;
use lsp_server::Response;
use lsp_types::{
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, FullDocumentDiagnosticReport,
    RelatedFullDocumentDiagnosticReport, WorkspaceDiagnosticReport,
    WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport,
    WorkspaceFullDocumentDiagnosticReport,
};

use crate::diagnostics;
use crate::server::make_response;
use crate::store::Store;
use crate::tags::TagIndex;

#[must_use]
pub fn handle_document_diagnostic(
    req: &lsp_server::Request,
    store: &Store,
    tag_index: &TagIndex,
) -> Response {
    let result = (|| -> Result<DocumentDiagnosticReportResult> {
        let params: lsp_types::DocumentDiagnosticParams =
            serde_json::from_value(req.params.clone())?;
        let uri = &params.text_document.uri;

        let items = store
            .get(uri)
            .map(|(_t, doc)| doc)
            .or_else(|| tag_index.workspace_doc(uri))
            .map(|doc| diagnostics::compute(doc, tag_index, uri))
            .unwrap_or_default();

        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items,
                },
            }),
        ))
    })();
    make_response(req, result)
}

#[must_use]
pub fn handle_workspace_diagnostic(req: &lsp_server::Request, tag_index: &TagIndex) -> Response {
    let result = (|| -> Result<WorkspaceDiagnosticReportResult> {
        let _params: lsp_types::WorkspaceDiagnosticParams =
            serde_json::from_value(req.params.clone())?;

        let items: Vec<WorkspaceDocumentDiagnosticReport> = tag_index
            .workspace_docs()
            .map(|(uri, doc)| {
                let diags = diagnostics::compute(doc, tag_index, uri);
                WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
                    uri: uri.clone(),
                    version: None,
                    full_document_diagnostic_report: FullDocumentDiagnosticReport {
                        result_id: None,
                        items: diags,
                    },
                })
            })
            .collect();

        Ok(WorkspaceDiagnosticReportResult::Report(
            WorkspaceDiagnosticReport { items },
        ))
    })();
    make_response(req, result)
}

use serde::{Deserialize, Serialize};

use crate::chunk::AstChunk;

/// Metadata template used for chunk output formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MetadataTemplate {
    /// No metadata.
    None,
    /// Default template with filepath, `chunk_size`, `line_count`, start/end line, `node_count`.
    #[default]
    Default,
    /// `CodeRAGBench` `RepoEval` template.
    CodeRagBenchRepoEval,
    /// `CodeRAGBench` SWE-bench Lite template.
    CodeRagBenchSwebenchLite,
}

/// Repository-level metadata used in chunk output formatting.
#[derive(Debug, Clone, Default)]
pub struct RepoMetadata {
    /// File path (used by Default template).
    pub filepath: String,
    /// Forward-slash–separated path tuple (used by `RepoEval` template).
    pub fpath_tuple: String,
    /// Repository name (used by `RepoEval` template).
    pub repo: String,
    /// Instance identifier (used by SWE-bench Lite template).
    pub instance_id: String,
    /// File name (used by SWE-bench Lite template).
    pub filename: String,
}

/// A serializable output code window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CodeWindow {
    /// Standard output: `{ "content": "...", "metadata": {...} }`
    Standard {
        /// The chunk text.
        content: String,
        /// Metadata dictionary.
        metadata: serde_json::Value,
    },
    /// SWE-bench Lite output: `{ "_id": "...", "title": "...", "text": "..." }`
    SwebenchLite {
        /// Unique identifier.
        _id: String,
        /// Title (filename).
        title: String,
        /// Chunk text.
        text: String,
    },
}

/// Build the metadata for a chunk according to the template.
pub fn build_metadata(
    chunk: &AstChunk,
    template: MetadataTemplate,
    repo_metadata: &RepoMetadata,
) -> CodeWindow {
    match template {
        MetadataTemplate::None => CodeWindow::Standard {
            content: chunk.text.clone(),
            metadata: serde_json::json!({}),
        },
        MetadataTemplate::Default => CodeWindow::Standard {
            content: chunk.text.clone(),
            metadata: serde_json::json!({
                "filepath": repo_metadata.filepath,
                "chunk_size": chunk.size,
                "line_count": chunk.line_count(),
                "start_line_no": chunk.start_line,
                "end_line_no": chunk.end_line,
                "node_count": chunk.node_count,
            }),
        },
        MetadataTemplate::CodeRagBenchRepoEval => {
            let fpath_tuple: Vec<String> = if repo_metadata.fpath_tuple.is_empty() {
                Vec::new()
            } else {
                repo_metadata
                    .fpath_tuple
                    .split('/')
                    .map(String::from)
                    .collect()
            };
            CodeWindow::Standard {
                content: chunk.text.clone(),
                metadata: serde_json::json!({
                    "fpath_tuple": fpath_tuple,
                    "repo": repo_metadata.repo,
                    "chunk_size": chunk.size,
                    "line_count": chunk.line_count(),
                    "start_line_no": chunk.start_line,
                    "end_line_no": chunk.end_line,
                    "node_count": chunk.node_count,
                }),
            }
        }
        MetadataTemplate::CodeRagBenchSwebenchLite => {
            let id = format!(
                "{}_{}-{}",
                repo_metadata.instance_id, chunk.start_line, chunk.end_line
            );
            CodeWindow::SwebenchLite {
                _id: id,
                title: repo_metadata.filename.clone(),
                text: chunk.text.clone(),
            }
        }
    }
}

/// Apply chunk expansion: prepend ancestry context header.
///
/// Format:
/// ```text
/// '''
/// <filepath>
/// <ancestor_0>
/// \t<ancestor_1>
/// \t\t<ancestor_2>
/// '''
/// <original chunk text>
/// ```
pub fn apply_chunk_expansion(
    chunk: &mut AstChunk,
    template: MetadataTemplate,
    repo_metadata: &RepoMetadata,
) {
    let filepath = match template {
        MetadataTemplate::Default => &repo_metadata.filepath,
        MetadataTemplate::CodeRagBenchRepoEval => &repo_metadata.fpath_tuple,
        MetadataTemplate::CodeRagBenchSwebenchLite => &repo_metadata.filename,
        MetadataTemplate::None => "",
    };

    let ancestors_text = chunk
        .ancestors
        .iter()
        .enumerate()
        .map(|(i, a)| format!("{}{a}", "\t".repeat(i)))
        .collect::<Vec<_>>()
        .join("\n");

    let mut expansion = String::from("'''\n");
    if !filepath.is_empty() {
        expansion.push_str(filepath);
        expansion.push('\n');
    }
    if !ancestors_text.is_empty() {
        expansion.push_str(&ancestors_text);
        expansion.push('\n');
    }
    expansion.push_str("'''");

    chunk.text = format!("{expansion}\n{}", chunk.text);
}

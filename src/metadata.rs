use std::collections::HashMap;

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

/// Options for the chunking process.
#[derive(Debug, Clone, Default)]
pub struct ChunkOptions {
    /// Number of `AstNode`s to overlap between adjacent windows (default: 0).
    pub chunk_overlap: usize,
    /// Whether to add ancestry context header to each chunk (default: false).
    pub chunk_expansion: bool,
    /// Repository-level metadata (e.g., filepath, repo, `instance_id`, filename).
    pub repo_metadata: HashMap<String, String>,
}

/// Build the metadata for a chunk according to the template.
pub fn build_metadata(
    chunk: &AstChunk,
    template: MetadataTemplate,
    repo_metadata: &HashMap<String, String>,
) -> CodeWindow {
    match template {
        MetadataTemplate::None => CodeWindow::Standard {
            content: chunk.text.clone(),
            metadata: serde_json::json!({}),
        },
        MetadataTemplate::Default => {
            let filepath = repo_metadata.get("filepath").cloned().unwrap_or_default();
            CodeWindow::Standard {
                content: chunk.text.clone(),
                metadata: serde_json::json!({
                    "filepath": filepath,
                    "chunk_size": chunk.size,
                    "line_count": chunk.line_count(),
                    "start_line_no": chunk.start_line,
                    "end_line_no": chunk.end_line,
                    "node_count": chunk.node_count,
                }),
            }
        }
        MetadataTemplate::CodeRagBenchRepoEval => {
            let fpath_tuple: Vec<String> = repo_metadata
                .get("fpath_tuple")
                .map(|s| s.split('/').map(String::from).collect())
                .unwrap_or_default();
            let repo = repo_metadata.get("repo").cloned().unwrap_or_default();
            CodeWindow::Standard {
                content: chunk.text.clone(),
                metadata: serde_json::json!({
                    "fpath_tuple": fpath_tuple,
                    "repo": repo,
                    "chunk_size": chunk.size,
                    "line_count": chunk.line_count(),
                    "start_line_no": chunk.start_line,
                    "end_line_no": chunk.end_line,
                    "node_count": chunk.node_count,
                }),
            }
        }
        MetadataTemplate::CodeRagBenchSwebenchLite => {
            let instance_id = repo_metadata
                .get("instance_id")
                .cloned()
                .unwrap_or_default();
            let filename = repo_metadata.get("filename").cloned().unwrap_or_default();
            let id = format!("{}_{}-{}", instance_id, chunk.start_line, chunk.end_line);
            CodeWindow::SwebenchLite {
                _id: id,
                title: filename,
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
    repo_metadata: &HashMap<String, String>,
) {
    let filepath = match template {
        MetadataTemplate::Default => repo_metadata.get("filepath").cloned().unwrap_or_default(),
        MetadataTemplate::CodeRagBenchRepoEval => repo_metadata
            .get("fpath_tuple")
            .cloned()
            .unwrap_or_default(),
        MetadataTemplate::CodeRagBenchSwebenchLite => {
            repo_metadata.get("filename").cloned().unwrap_or_default()
        }
        MetadataTemplate::None => String::new(),
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
        expansion.push_str(&filepath);
        expansion.push('\n');
    }
    if !ancestors_text.is_empty() {
        expansion.push_str(&ancestors_text);
        expansion.push('\n');
    }
    expansion.push_str("'''");

    chunk.text = format!("{expansion}\n{}", chunk.text);
}

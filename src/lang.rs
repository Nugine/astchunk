//! Language definitions and tree-sitter bindings used by the chunking pipeline.

/// Supported programming languages for AST-based chunking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Python
    Python,
    /// Java
    Java,
    /// C++
    Cpp,
    /// Rust
    Rust,
    /// C#
    CSharp,
    /// TypeScript / TSX
    TypeScript,
}

impl Language {
    /// Detect language from file extension.
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "py" => Some(Self::Python),
            "java" => Some(Self::Java),
            "cpp" | "cc" | "cxx" | "c++" | "h" | "hpp" | "hxx" | "hh" => Some(Self::Cpp),
            "rs" => Some(Self::Rust),
            "cs" => Some(Self::CSharp),
            "ts" | "tsx" => Some(Self::TypeScript),
            _ => None,
        }
    }

    /// Returns the tree-sitter [`Language`](tree_sitter::Language) for this programming language.
    #[must_use]
    pub fn ts_language(self) -> tree_sitter::Language {
        match self {
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TSX.into(),
        }
    }

    /// Returns the tree-sitter node type strings that represent class or function
    /// definitions in this language. Used for building ancestor paths.
    #[must_use]
    pub fn ancestor_node_types(self) -> &'static [&'static str] {
        match self {
            Self::Python => &["class_definition", "function_definition"],
            Self::Cpp => &[
                "class_specifier",
                "function_definition",
                "namespace_definition",
                "struct_specifier",
            ],
            Self::Rust => &[
                "impl_item",
                "function_item",
                "struct_item",
                "enum_item",
                "trait_item",
                "mod_item",
            ],
            Self::Java | Self::CSharp => &[
                "class_declaration",
                "method_declaration",
                "constructor_declaration",
                "interface_declaration",
            ],
            Self::TypeScript => &[
                "class_declaration",
                "method_definition",
                "function_declaration",
                "arrow_function",
                "interface_declaration",
            ],
        }
    }

    /// Returns the tree-sitter node type string for the root/module node in this language.
    #[must_use]
    pub fn root_node_type(self) -> &'static str {
        match self {
            Self::Python => "module",
            Self::Cpp => "translation_unit",
            Self::Rust => "source_file",
            Self::Java | Self::TypeScript => "program",
            Self::CSharp => "compilation_unit",
        }
    }
}

/// Supported programming languages for AST-based chunking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Python
    #[cfg(feature = "python")]
    Python,
    /// Java
    #[cfg(feature = "java")]
    Java,
    /// C#
    #[cfg(feature = "csharp")]
    CSharp,
    /// TypeScript / TSX
    #[cfg(feature = "typescript")]
    TypeScript,
}

impl Language {
    /// Returns the tree-sitter [`Language`](tree_sitter::Language) for this programming language.
    #[must_use]
    pub fn ts_language(self) -> tree_sitter::Language {
        match self {
            #[cfg(feature = "python")]
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            #[cfg(feature = "java")]
            Self::Java => tree_sitter_java::LANGUAGE.into(),
            #[cfg(feature = "csharp")]
            Self::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            #[cfg(feature = "typescript")]
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TSX.into(),
        }
    }

    /// Returns the tree-sitter node type strings that represent class or function
    /// definitions in this language. Used for building ancestor paths.
    #[must_use]
    pub fn ancestor_node_types(self) -> &'static [&'static str] {
        match self {
            #[cfg(feature = "python")]
            Self::Python => &["class_definition", "function_definition"],
            #[cfg(feature = "java")]
            Self::Java => &[
                "class_declaration",
                "method_declaration",
                "constructor_declaration",
                "interface_declaration",
            ],
            #[cfg(feature = "csharp")]
            Self::CSharp => &[
                "class_declaration",
                "method_declaration",
                "constructor_declaration",
                "interface_declaration",
            ],
            #[cfg(feature = "typescript")]
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
            #[cfg(feature = "python")]
            Self::Python => "module",
            #[cfg(feature = "java")]
            Self::Java => "program",
            #[cfg(feature = "csharp")]
            Self::CSharp => "compilation_unit",
            #[cfg(feature = "typescript")]
            Self::TypeScript => "program",
        }
    }
}

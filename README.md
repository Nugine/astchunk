# astchunk

A Rust implementation of AST-based code chunking, reproducing the paper:

> [cAST: Enhancing Code Retrieval-Augmented Generation with Structural Chunking via Abstract Syntax Tree](https://arxiv.org/abs/2506.15655)  
> Yilin Zhang et al.

Original Python implementation: [yilinjz/astchunk](https://github.com/yilinjz/astchunk)

ASTChunk splits source code into chunks while respecting syntactic structure and semantic boundaries, making it suitable for code RAG pipelines.

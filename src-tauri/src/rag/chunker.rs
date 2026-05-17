use super::model::CodeChunk;
use tree_sitter::Parser;

// ---------------------------------------------------------------------------
// Language detection
// ---------------------------------------------------------------------------

pub fn detect_language(file_path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())?;
    match ext {
        "rs" => Some("rust"),
        "ts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "js" | "jsx" | "mjs" => Some("javascript"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "cs" => Some("c_sharp"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tree-sitter helpers
// ---------------------------------------------------------------------------

fn get_language(lang: &str) -> Option<tree_sitter::Language> {
    match lang {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "c_sharp" => Some(tree_sitter_c_sharp::LANGUAGE.into()),
        _ => None,
    }
}

fn is_substantial(kind: &str, language: &str) -> bool {
    match language {
        "rust" => matches!(
            kind,
            "function_item"
                | "struct_item"
                | "enum_item"
                | "impl_item"
                | "trait_item"
                | "mod_item"
        ),
        "typescript" | "tsx" | "javascript" => matches!(
            kind,
            "function_declaration"
                | "class_declaration"
                | "export_statement"
                | "interface_declaration"
                | "type_alias_declaration"
                | "enum_declaration"
        ),
        "python" => matches!(
            kind,
            "function_definition" | "class_definition" | "decorated_definition"
        ),
        "go" => matches!(
            kind,
            "function_declaration" | "method_declaration" | "type_declaration"
        ),
        "java" | "c_sharp" => matches!(
            kind,
            "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "method_declaration"
                | "struct_declaration"
                | "namespace_declaration"
        ),
        _ => false,
    }
}

fn make_chunk(
    file_path: &str,
    start_line: usize,
    end_line: usize,
    text: &str,
    language: &str,
) -> CodeChunk {
    CodeChunk {
        file_path: file_path.to_string(),
        start_line,
        end_line,
        content: format!(
            "// File: {file_path} | Lines: {start_line}-{end_line}\n{text}"
        ),
        language: language.to_string(),
    }
}

fn flush_small_buf(
    chunks: &mut Vec<CodeChunk>,
    buf: &mut Vec<String>,
    file_path: &str,
    start_line: usize,
    end_line: usize,
    language: &str,
) {
    if buf.is_empty() {
        return;
    }
    let text = buf.join("\n");
    chunks.push(make_chunk(file_path, start_line, end_line, &text, language));
    buf.clear();
}

// ---------------------------------------------------------------------------
// AST-aware chunking
// ---------------------------------------------------------------------------

/// Chunk a source file using tree-sitter AST awareness.
/// Falls back to `chunk_lines(content, file_path, 50, 10)` if language is
/// unknown or the file fails to parse.
pub fn chunk_file(
    content: &str,
    file_path: &str,
    language: &str,
    max_chunk_chars: usize,
) -> Vec<CodeChunk> {
    let ts_language = match get_language(language) {
        Some(l) => l,
        None => return chunk_lines(content, file_path, 50, 10),
    };

    let mut parser = Parser::new();
    if parser.set_language(&ts_language).is_err() {
        return chunk_lines(content, file_path, 50, 10);
    }

    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return chunk_lines(content, file_path, 50, 10),
    };

    let root = tree.root_node();
    let lines: Vec<&str> = content.lines().collect();
    let mut chunks: Vec<CodeChunk> = Vec::new();

    // Small-node accumulator
    let mut small_buf: Vec<String> = Vec::new();
    let mut small_start: usize = 0; // 1-indexed
    let mut small_end: usize = 0; // 1-indexed

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.is_extra() || child.kind() == "comment" {
            // Treat as small node
        }

        let node_start = child.start_position().row; // 0-indexed
        let node_end = child.end_position().row; // 0-indexed (inclusive)
        let start_1 = node_start + 1;
        let end_1 = node_end + 1;

        let node_text: String = lines
            .get(node_start..=node_end)
            .map(|ls| ls.join("\n"))
            .unwrap_or_default();

        if is_substantial(child.kind(), language) {
            // Flush any accumulated small nodes first
            flush_small_buf(
                &mut chunks,
                &mut small_buf,
                file_path,
                small_start,
                small_end,
                language,
            );

            // Emit the substantial node — recurse if too large
            if node_text.len() > max_chunk_chars {
                // Try to split by recursing into children
                let sub = chunk_node_children(
                    &child,
                    content,
                    file_path,
                    language,
                    max_chunk_chars,
                    &lines,
                );
                if sub.is_empty() {
                    // Emit as one big chunk anyway
                    chunks.push(make_chunk(file_path, start_1, end_1, &node_text, language));
                } else {
                    chunks.extend(sub);
                }
            } else {
                chunks.push(make_chunk(file_path, start_1, end_1, &node_text, language));
            }
        } else {
            // Accumulate into small buffer
            if small_buf.is_empty() {
                small_start = start_1;
            }
            small_end = end_1;
            small_buf.push(node_text);

            // Flush if buffer is getting too large
            let buf_len: usize = small_buf.iter().map(|s| s.len() + 1).sum();
            if buf_len > max_chunk_chars {
                flush_small_buf(
                    &mut chunks,
                    &mut small_buf,
                    file_path,
                    small_start,
                    small_end,
                    language,
                );
            }
        }
    }

    // Flush any remaining small nodes
    flush_small_buf(
        &mut chunks,
        &mut small_buf,
        file_path,
        small_start,
        small_end,
        language,
    );

    if chunks.is_empty() {
        // Nothing was emitted — fall back
        chunk_lines(content, file_path, 50, 10)
    } else {
        chunks
    }
}

/// Recurse into a node's children to split an oversized substantial node.
fn chunk_node_children<'a>(
    node: &tree_sitter::Node<'a>,
    _content: &str,
    file_path: &str,
    language: &str,
    max_chunk_chars: usize,
    lines: &[&str],
) -> Vec<CodeChunk> {
    let mut chunks = Vec::new();
    let mut buf: Vec<String> = Vec::new();
    let mut buf_start = 0usize;
    let mut buf_end = 0usize;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let node_start = child.start_position().row;
        let node_end = child.end_position().row;
        let start_1 = node_start + 1;
        let end_1 = node_end + 1;

        let child_text: String = lines
            .get(node_start..=node_end)
            .map(|ls| ls.join("\n"))
            .unwrap_or_default();

        if child_text.is_empty() {
            continue;
        }

        // Check if adding this child would overflow the buffer
        let new_len: usize = buf.iter().map(|s| s.len() + 1).sum::<usize>() + child_text.len();
        if !buf.is_empty() && new_len > max_chunk_chars {
            // Flush current buffer
            let text = buf.join("\n");
            chunks.push(make_chunk(file_path, buf_start, buf_end, &text, language));
            buf.clear();
        }

        if buf.is_empty() {
            buf_start = start_1;
        }
        buf_end = end_1;
        buf.push(child_text);
    }

    if !buf.is_empty() {
        let text = buf.join("\n");
        chunks.push(make_chunk(file_path, buf_start, buf_end, &text, language));
    }

    chunks
}

pub fn chunk_lines(
    content: &str,
    file_path: &str,
    window: usize,
    overlap: usize,
) -> Vec<CodeChunk> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return vec![];
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    let step = window.saturating_sub(overlap).max(1);

    while start < lines.len() {
        let end = (start + window).min(lines.len());
        let start_line = start + 1;
        let end_line = end;
        let chunk_content = lines[start..end].join("\n");

        chunks.push(CodeChunk {
            file_path: file_path.to_string(),
            start_line,
            end_line,
            content: format!(
                "// File: {file_path} | Lines: {start_line}-{end_line}\n{chunk_content}"
            ),
            language: "text".to_string(),
        });

        if end >= lines.len() {
            break;
        }
        start += step;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // detect_language tests
    // -----------------------------------------------------------------------

    #[test]
    fn detect_language_by_extension() {
        assert_eq!(detect_language("src/main.rs"), Some("rust"));
        assert_eq!(detect_language("App.tsx"), Some("tsx"));
        assert_eq!(detect_language("index.ts"), Some("typescript"));
        assert_eq!(detect_language("script.js"), Some("javascript"));
        assert_eq!(detect_language("app.py"), Some("python"));
        assert_eq!(detect_language("main.go"), Some("go"));
        assert_eq!(detect_language("Main.java"), Some("java"));
        assert_eq!(detect_language("Program.cs"), Some("c_sharp"));
        assert_eq!(detect_language("README.md"), None);
        assert_eq!(detect_language("config.yaml"), None);
    }

    // -----------------------------------------------------------------------
    // chunk_file tests
    // -----------------------------------------------------------------------

    #[test]
    fn chunk_file_rust_functions() {
        let source = r#"use std::io;

const MAX: usize = 100;

fn hello() {
    println!("hello");
}

fn goodbye() {
    println!("goodbye");
}
"#;
        let chunks = chunk_file(source, "lib.rs", "rust", 1600);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].content.contains("use std::io"));
        let all_content: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(all_content.contains("fn hello()"));
        assert!(all_content.contains("fn goodbye()"));
    }

    #[test]
    fn chunk_file_falls_back_for_unknown_language() {
        let content = "line1\nline2\nline3";
        let chunks = chunk_file(content, "config.yaml", "yaml", 1600);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].language, "text");
    }

    #[test]
    fn chunk_file_typescript() {
        let source = r#"import { useState } from "react";

export function App() {
    return <div>hello</div>;
}
"#;
        let chunks = chunk_file(source, "App.tsx", "tsx", 1600);
        assert!(!chunks.is_empty());
        let all: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(all.contains("import"));
        assert!(all.contains("App"));
    }

    #[test]
    fn chunk_file_splits_large_nodes() {
        let mut source = String::from("impl Foo {\n");
        for i in 0..20 {
            source.push_str(&format!("    fn method_{i}() {{ println!(\"{i}\"); }}\n"));
        }
        source.push_str("}\n");
        let chunks = chunk_file(&source, "foo.rs", "rust", 100);
        assert!(chunks.len() > 1);
    }

    #[test]
    fn chunk_lines_basic() {
        let content = (1..=100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_lines(&content, "test.txt", 50, 10);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 50);
        assert_eq!(chunks[1].start_line, 41);
        assert_eq!(chunks[1].end_line, 90);
        assert_eq!(chunks[2].start_line, 81);
        assert_eq!(chunks[2].end_line, 100);
        assert_eq!(chunks[0].language, "text");
    }

    #[test]
    fn chunk_lines_small_file() {
        let content = "line 1\nline 2\nline 3";
        let chunks = chunk_lines(content, "small.txt", 50, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
    }

    #[test]
    fn chunk_lines_includes_header() {
        let content = "fn main() {}\nlet x = 1;";
        let chunks = chunk_lines(content, "src/main.rs", 50, 10);
        assert!(chunks[0].content.starts_with("// File: src/main.rs | Lines: 1-2"));
    }

    #[test]
    fn chunk_lines_empty_content() {
        let chunks = chunk_lines("", "empty.txt", 50, 10);
        assert!(chunks.is_empty());
    }
}

use super::model::CodeChunk;

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

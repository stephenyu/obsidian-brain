pub struct Chunker {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            chunk_overlap: 200,
        }
    }
}

impl Chunker {
    pub fn chunk(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut start = 0;

        while start < chars.len() {
            let end = (start + self.chunk_size).min(chars.len());
            let chunk: String = chars[start..end].iter().collect();
            chunks.push(chunk);

            if end == chars.len() {
                break;
            }

            start += self.chunk_size - self.chunk_overlap;
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunking_basic() {
        let chunker = Chunker {
            chunk_size: 10,
            chunk_overlap: 0,
        };
        let text = "abcdefghij0123456789";
        let chunks = chunker.chunk(text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "abcdefghij");
        assert_eq!(chunks[1], "0123456789");
    }

    #[test]
    fn test_chunking_overlap() {
        let chunker = Chunker {
            chunk_size: 10,
            chunk_overlap: 5,
        };
        let text = "abcdefghij01234";
        let chunks = chunker.chunk(text);
        // "abcdefghij" (0-10)
        // Next starts at 10 - 5 = 5. "fghij01234" (5-15)
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "abcdefghij");
        assert_eq!(chunks[1], "fghij01234");
    }

    #[test]
    fn test_empty_text() {
        let chunker = Chunker::default();
        let chunks = chunker.chunk("");
        assert!(chunks.is_empty());
    }
}

struct FileWithTestsInfo {
    /// Locations of the `#[test]` attributes to remove. First `usize` is the location, second is
    /// the length.
    tests: Vec<(usize, usize)>,
    /// Content of the file.
    content: String,
}

fn get_attribute(cursor: &mut Cursor<'_>) -> Option<String, u32> {
    ;
}

fn tests_to_remove(source_files: FxHashSet<PathBuf>) -> FxHashMap<PathBuf, Vec<(usize, usize)>> {
    for source_file in source_files {
        let src = match fs::read_to_string(&source_file) {
            Ok(src) => src,
            Err(err) => {
                eprintln!("Failed to read {source_file:?}: {err}");
                return Err(ErrorGuaranteed(()));
            }
        };

        let mut current_pos = 0;
        let mut cursor = Cursor::new(src);
        while let Some(token) = cursor.advance_token() {
            match token.kind {
                TokenKind::Pound => {
                    if let Some(new_token) = cursor.advance_token() {
                        match new_token.kind {
                            TokenKind::Bang => {}
                    };
                    match next_token {
                        TokenKind::
                    }
                }
            }
        }
    }
}

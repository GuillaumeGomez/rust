use crate::diagnostics::{CheckId, DiagCtx, RunningCheck};

use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub fn check(root_path: &Path, diag_ctx: DiagCtx) {
    let path = &root_path.join("compiler");
    let mut check = diag_ctx.start_check(CheckId::new("error_messages").path(path));
    let regex = Regex::new(r#"(?:(?:#\[(diag|label|suggestion|note|help|multipart_suggestion)\()|(?:fluent(_generated)?::))\s*(?<id>[a-z0-9_]+)\s*[\n,\)}]"#).unwrap();

    for dir in std::fs::read_dir(path).unwrap() {
        let Ok(dir) = dir else { continue };
        let dir = dir.path();
        let messages_file = dir.join("messages.ftl");

        if messages_file.is_file() {
            check_if_messages_are_used(&mut check, &dir.join("src"), &messages_file, &regex);
        }
    }
}

fn check_if_messages_are_used(check: &mut RunningCheck, src_path: &Path, messages_file: &Path, regex: &Regex) {
    // First we retrieve all error messages ID.
    let content = std::fs::read_to_string(messages_file).expect("failed to read file");
    let mut ids: HashMap<&str, bool> = HashMap::new();

    for line in content.lines() {
        if line.len() == line.trim_start().len()
            && line.contains('=')
            && let Some(id) = line.split('=').next()
        {
            let id = id.trim();
            if id.starts_with('-') || id.contains('=') || id.contains('"') {
                // Used in the same file by other error messages.
                // FIXME: Handle this case too.
                continue;
            }
            ids.insert(id, false);
        }
    }
    assert!(!ids.is_empty());

    let skip = |f: &Path, is_dir: bool| !is_dir && !f.extension().is_some_and(|ext| ext == "rs");
    crate::walk::walk(
        src_path,
        skip,
        &mut |_path: &_, content: &str| {
            for cap in regex.captures_iter(content) {
                let id = &cap["id"];
                match ids.get_mut(id) {
                    // Error message IDs can be used more than once.
                    Some(found) => *found = true,
                    None => {
                        // FIXME: Why are there so many error message IDs not declared in `.ftl`
                        // files?
                        // let path = _path.path();
                        // check.error(format!("unknown error message ID `{id}` in `{}`", path.display())),
                    }
                }
            }
        },
    );
    for (id, found) in ids {
        if !found {
            check.error(format!("unused message ID `{id}` from `{}`", messages_file.display()));
        }
    }
}

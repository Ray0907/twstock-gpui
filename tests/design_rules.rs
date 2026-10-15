use std::{fs, path::Path};

// No exceptions: platform/GPU sizing and component styling belong in design too.
const WHITELIST: &[(&str, &str)] = &[];

fn violations(source: &str) -> Vec<(usize, &'static str)> {
    use proc_macro2::{Delimiter, TokenStream, TokenTree};
    fn method(name: &str) -> Option<&'static str> {
        match name {
            "text_color" => return Some(".text_color("),
            "bg" => return Some(".bg("),
            "style" | "theme" | "padding" | "paddings" | "margin" | "margins" | "line_height"
            | "w" | "h" | "width" | "height" | "size" | "min_w" | "min_h" | "max_w" | "max_h" => {
                return Some("visual method");
            }
            _ => {}
        }
        for (prefix, rule) in [
            ("font_", ".font_"),
            ("text_", ".text_"),
            ("rounded", ".rounded"),
            ("border_", ".border_"),
            ("shadow", ".shadow"),
            ("w_", "width"),
            ("h_", "height"),
            ("size_", "size"),
            ("min_w_", "width"),
            ("min_h_", "height"),
            ("max_w_", "width"),
            ("max_h_", "height"),
        ] {
            if name.starts_with(prefix) {
                return Some(rule);
            }
        }
        for stem in [
            "p", "px", "py", "pt", "pb", "pl", "pr", "m", "mx", "my", "mt", "mb", "ml", "mr",
            "gap", "gap_x", "gap_y", "space_x", "space_y",
        ] {
            if name == stem
                || name
                    .strip_prefix(stem)
                    .is_some_and(|tail| tail.starts_with('_'))
            {
                return Some("spacing");
            }
        }
        None
    }
    fn walk(stream: TokenStream, found: &mut Vec<(usize, &'static str)>) {
        let tokens: Vec<_> = stream.into_iter().collect();
        for (i, token) in tokens.iter().enumerate() {
            if let TokenTree::Group(g) = token {
                walk(g.stream(), found);
            }
            let rule = match token {
                TokenTree::Ident(name) => {
                    let name = name.to_string();
                    let name = name.strip_prefix("r#").unwrap_or(&name);
                    let call = matches!(tokens.get(i+1),Some(TokenTree::Group(g)) if g.delimiter()==Delimiter::Parenthesis);
                    let is_method = matches!(i.checked_sub(1).and_then(|j|tokens.get(j)),Some(TokenTree::Punct(p)) if p.as_char()=='.');
                    if call && is_method {
                        method(name)
                    } else if call {
                        match name {
                            "px" => Some("px("),
                            "rgb" | "rgba" | "hsla" => Some("color constructor"),
                            "rems" => Some("rems("),
                            _ => None,
                        }
                    } else if ["FontWeight", "Theme", "ActiveTheme"].contains(&name) {
                        Some("theme/font access")
                    } else {
                        None
                    }
                }
                TokenTree::Literal(value) => {
                    let value = value.to_string();
                    ["▲", "▼", "{:+"].into_iter().find(|p| value.contains(p))
                }
                _ => None,
            };
            if let Some(rule) = rule {
                found.push((token.span().start().line, rule));
            }
        }
    }
    let stream = source
        .parse::<TokenStream>()
        .expect("source must tokenize; do not silently skip malformed Rust");
    let mut found = Vec::new();
    walk(stream, &mut found);
    found
}

fn scan(dir: &Path, root: &Path, failures: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path == root.join("src/design") {
            continue;
        }
        if path.is_dir() {
            scan(&path, root, failures);
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
        if WHITELIST
            .iter()
            .any(|(file, reason)| *file == relative && !reason.is_empty())
        {
            continue;
        }
        for (line, rule) in violations(&fs::read_to_string(&path).unwrap()) {
            failures.push(format!("{relative}:{line}: forbidden {rule}"));
        }
    }
}

#[test]
fn visual_decisions_only_live_in_design() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut failures = Vec::new();
    scan(&root.join("src"), root, &mut failures);
    failures.sort();
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn r7_scanner_catches_spacing_and_split_calls_but_not_comments() {
    for method in [
        "p_3",
        "px_3",
        "py_3",
        "pt_3",
        "pb_3",
        "pl_3",
        "pr_3",
        "m_3",
        "mx_3",
        "my_3",
        "mt_3",
        "mb_3",
        "ml_3",
        "mr_3",
        "gap_3",
        "gap_x_3",
        "gap_y_3",
        "space_x_3",
        "space_y_3",
    ] {
        assert!(
            !violations(&format!("div().{method}();")).is_empty(),
            "missed {method}"
        );
    }
    for method in [
        "min_w", "min_h", "max_w", "max_h", "paddings", "margins", "r#bg",
    ] {
        assert!(
            !violations(&format!("div().{method}(value);")).is_empty(),
            "{method}"
        );
    }
    assert!(!violations("div().bg\n (color);").is_empty());
    assert!(!violations("div() . bg /* comment */ (color);").is_empty());
    assert!(violations("// div().bg(color);\nlet s=\".pt_3().bg(red)\";").is_empty());
}
#[test]
fn detector_rejects_styles_and_ignores_comments() {
    assert_eq!(
        violations("// .text_sm()\nvalue.font_weight(weight)\npx(13.)\nvalue.text_color(color)"),
        vec![(2, ".font_"), (3, "px("), (4, ".text_color(")]
    );
}

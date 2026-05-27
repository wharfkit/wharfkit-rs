pub fn snake_ident(abi_name: &str) -> String {
    let s = abi_name.replace('.', "_");
    if is_rust_keyword(&s) {
        format!("r#{s}")
    } else {
        s
    }
}

pub fn camel_ident(abi_name: &str) -> String {
    let cleaned = abi_name.replace('.', "_");
    let mut out = String::new();
    let mut capitalize_next = true;
    for ch in cleaned.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            out.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn is_rust_keyword(s: &str) -> bool {
    matches!(
        s,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "dyn"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
            | "gen"
    )
}

pub fn check_no_collisions<I, F>(names: I, mut munge: F) -> Result<(), String>
where
    I: IntoIterator<Item = String>,
    F: FnMut(&str) -> String,
{
    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for name in names {
        let munged = munge(&name);
        if let Some(existing) = seen.get(&munged) {
            return Err(format!(
                "ident collision: ABI names '{existing}' and '{name}' both munge to '{munged}'"
            ));
        }
        seen.insert(munged, name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_ident_passthrough() {
        assert_eq!(snake_ident("transfer"), "transfer");
        assert_eq!(snake_ident("account_row"), "account_row");
    }

    #[test]
    fn snake_ident_keyword_raw() {
        assert_eq!(snake_ident("type"), "r#type");
        assert_eq!(snake_ident("move"), "r#move");
        assert_eq!(snake_ident("async"), "r#async");
        assert_eq!(snake_ident("gen"), "r#gen");
    }

    #[test]
    fn snake_ident_dot_underscore() {
        assert_eq!(snake_ident("powup.order"), "powup_order");
    }

    #[test]
    fn camel_ident_basic() {
        assert_eq!(camel_ident("account_row"), "AccountRow");
        assert_eq!(camel_ident("transfer"), "Transfer");
    }

    #[test]
    fn camel_ident_dot() {
        assert_eq!(camel_ident("powup.order"), "PowupOrder");
    }

    #[test]
    fn collision_detected() {
        let names = vec!["foo_bar".to_string(), "foo.bar".to_string()];
        let result = check_no_collisions(names, snake_ident);
        assert!(result.is_err());
    }

    #[test]
    fn no_collision() {
        let names = vec!["foo".to_string(), "bar".to_string(), "baz".to_string()];
        assert!(check_no_collisions(names, snake_ident).is_ok());
    }
}

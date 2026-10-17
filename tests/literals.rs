use typescript_parser::{Value, parse};

fn json(source: &str) -> serde_json::Value {
    let ast = parse(source).unwrap();
    let mut output = Vec::new();
    ast.write_json(&mut output, false).unwrap();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn numeric_literals_keep_compiler_text_flags_and_spans_for_both_cases() {
    // Expected node fields from the pinned compiler projection. Hexadecimal
    // E/e digits are radix digits, not the decimal scientific flag.
    for (raw, text, flags) in [
        ("0", "0", 0),
        ("123", "123", 0),
        ("1.5", "1.5", 0),
        (".5", "0.5", 0),
        ("1.", "1", 0),
        ("1e2", "100", 16),
        ("1E+2", "100", 16),
        ("1E-2", "0.01", 16),
        ("0xDeAD", "57005", 64),
        ("0XDEAD", "57005", 64),
        ("0b101", "5", 128),
        ("0B101", "5", 128),
        ("0o17", "15", 256),
        ("0O17", "15", 256),
        ("1_000", "1000", 512),
        ("1e1_0", "10000000000", 528),
        ("1E1_0", "10000000000", 528),
        ("0xDE_AD", "57005", 576),
        ("0XDE_AD", "57005", 576),
        ("0b10_10", "10", 640),
        ("0B10_10", "10", 640),
        ("0o1_7", "15", 768),
        ("0O1_7", "15", 768),
    ] {
        let source = format!("const x = {raw};");
        let value = json(&source);
        let literal = &value["statements"][0]["declarationList"]["declarations"][0]["initializer"];
        assert_eq!(literal["kind"], 9, "{raw}");
        assert_eq!(literal["text"], text, "{raw}");
        assert_eq!(literal["numericLiteralFlags"], flags, "{raw}");
        assert_eq!(literal["pos"], 9, "{raw}");
        assert_eq!(literal["end"], 10 + raw.len(), "{raw}");
    }
    for (raw, text) in [("123n", "123n"), ("0XDEADn", "0xdeadn")] {
        let value = json(&format!("const x = {raw};"));
        let literal = &value["statements"][0]["declarationList"]["declarations"][0]["initializer"];
        assert_eq!(literal["kind"], 10);
        assert_eq!(literal["text"], text);
        assert_eq!(literal.get("numericLiteralFlags"), None);
    }
}

#[test]
fn binary64_literal_text_uses_the_native_shortest_roundtrip_and_exponent_windows() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-property-lookups/literals.json"
    ))
    .unwrap();
    for row in rows {
        let raw = row["raw"].as_str().unwrap();
        let source = format!("const x = {raw};");
        let value = json(&source);
        let literal = &value["statements"][0]["declarationList"]["declarations"][0]["initializer"];
        assert_eq!(literal["kind"], 9, "{raw}");
        assert_eq!(literal["text"], row["text"], "{raw}");
        assert_eq!(literal["pos"], 9, "{raw}");
        assert_eq!(literal["end"], 10 + raw.len(), "{raw}");
    }
}

#[test]
fn hexadecimal_escapes_require_ascii_hex_digits() {
    for escape in [
        r"\u+041",
        r"\x+1",
        r"\u{+41}",
        r"\u12G4",
        r"\xG1",
        r"\u{}",
        r"\u{110000}",
        r"\u{FFFFFFFFFFFFFFFFFFFFFFFF}",
    ] {
        for source in [
            format!(r#"const value = "{escape}";"#),
            format!("`{escape}`;"),
        ] {
            assert!(parse(&source).is_err(), "accepted invalid escape: {source}");
        }
    }
    let value = json(r#"const value = "\u{000000000000000000000000000041}";"#);
    assert_eq!(
        value["statements"][0]["declarationList"]["declarations"][0]["initializer"]["text"],
        "A"
    );
}

#[test]
fn tagged_literals_preserve_cooking_and_flags_around_invalid_escapes() {
    for (raw, text, flags, extended) in [
        (r"\u0041\xGG", r"A\xGG", 3072, false),
        (r"\xGG\u0041", r"\xGGA", 3072, false),
        (r"\u{41}\xGG\u0042", r"A\xGGB", 3080, true),
        (r"\x41\u+041\x42", r"A\u+041B", 6144, false),
        (r"\u{+41}\u{42}", r"\u{+41}B", 2056, true),
        (r"\1\u0041", r"\1A", 3072, false),
        (r"\\u{41}\x41", r"\u{41}A", 4096, false),
    ] {
        let source = format!("tag`{raw}`;");
        let value = json(&source);
        let template = &value["statements"][0]["expression"]["template"];
        assert_eq!(template["text"], text, "{source}");
        assert_eq!(template["rawText"], raw, "{source}");
        assert_eq!(template["templateFlags"], flags, "{source}");
        assert_eq!(
            template.get("hasExtendedUnicodeEscape"),
            extended.then_some(&serde_json::Value::Bool(true)),
            "{source}"
        );
    }
}

#[test]
fn tagged_template_segments_keep_their_own_escape_metadata() {
    let value = json(r"tag`\u0041\xGG${value}\x42\u+041${value}\u{43}\xGG`;");
    let template = &value["statements"][0]["expression"]["template"];
    assert_eq!(template["head"]["text"], r"A\xGG");
    assert_eq!(template["head"]["templateFlags"], 3072);
    let middle = &template["templateSpans"][0]["literal"];
    assert_eq!(middle["text"], r"B\u+041");
    assert_eq!(middle["templateFlags"], 6144);
    let tail = &template["templateSpans"][1]["literal"];
    assert_eq!(tail["text"], r"C\xGG");
    assert_eq!(tail["templateFlags"], 2056);
    assert_eq!(tail["hasExtendedUnicodeEscape"], true);
}

#[test]
fn tagged_invalid_escapes_preserve_unpaired_utf16_surrogates() {
    let ast = parse(r"tag`\uD800\xGG`; ").unwrap();
    assert!(ast.walk().map(|id| ast.node(id)).any(|node| {
        matches!(node.field("text"), Some(Value::Utf16(units)) if units == &[0xD800, 0x5C, 0x78, 0x47, 0x47])
    }));
    let mut output = Vec::new();
    ast.write_json(&mut output, false).unwrap();
    assert!(String::from_utf8(output).unwrap().contains("\\ud800"));
}

#[test]
fn public_literal_projection_keeps_utf16_across_native_scanner_pairing_boundaries() {
    // Native scanner token bytes distinguish continued halves; the public AST
    // keeps its established UTF-16 projection for every string/template token.
    for raw in [
        r"\uD800\uDC00".to_owned(),
        r"\uD800\u{DC00}".to_owned(),
        r"\u{D800}\uDC00".to_owned(),
        r"\u{D800}\u{DC00}".to_owned(),
    ]
    .into_iter()
    .chain(
        ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"].map(|line| format!("\\uD800\\{line}\\uDC00")),
    ) {
        for source in [
            format!("const value = \"{raw}\";"),
            format!("`{raw}`;"),
            format!("`{raw}${{0}}{raw}${{1}}{raw}`;"),
        ] {
            let ast = parse(&source).unwrap();
            let texts: Vec<_> = ast
                .walk()
                .filter_map(|id| match ast.node(id).field("text") {
                    Some(Value::Text(text)) if text.as_ref() == "𐀀" => Some(text),
                    _ => None,
                })
                .collect();
            assert_eq!(
                texts.len(),
                if source.contains("${0}") { 3 } else { 1 },
                "{source:?}"
            );
        }
    }
    for (raw, units) in [
        (r"\uD800", vec![0xD800]),
        (r"\uDC00", vec![0xDC00]),
        (r"\uDC00\uD800", vec![0xDC00, 0xD800]),
    ] {
        for source in [format!("\"{raw}\";"), format!("`{raw}`;")] {
            let ast = parse(&source).unwrap();
            assert!(
                ast.walk().any(|id| matches!(ast.node(id).field("text"),
                Some(Value::Utf16(value)) if value == &units)),
                "{source}"
            );
        }
    }
}

#[test]
fn ordinary_and_jsx_string_literals_keep_their_native_scanner_dialects() {
    let source = include_str!("../fixtures/types/native-string-values/dialects.tsx");
    let ast = typescript_parser::parse_with_options(
        source,
        typescript_parser::ParseOptions::for_file("dialects.tsx"),
    )
    .unwrap();
    let texts: Vec<_> = ast
        .walk()
        .filter_map(|id| {
            let node = ast.node(id);
            if node.kind != "StringLiteral" {
                return None;
            }
            match node.field("text") {
                Some(Value::Text(text)) => Some(text.as_ref()),
                other => panic!("unexpected literal projection: {other:?}"),
            }
        })
        .collect();
    // Native AST text leaves JSX entities and backslashes for the JSX owner.
    assert_eq!(texts, ["A\n&amp;", r"\u0041\n&amp;"]);
}

#[test]
fn unterminated_tokens_fail_before_literal_delimiter_projection() {
    for source in ["\"", "'", "`", "\"\\", "`\\", "`${0", "`${0}", "`${0}a${1"] {
        assert!(
            parse(source).is_err(),
            "accepted unterminated token: {source:?}"
        );
    }
}

#[test]
fn jsdoc_recovery_accepts_unterminated_literals_without_corrupting_the_final_ast() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-string-values/jsdoc-malformed.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 38);
    for row in rows {
        let source = row["source"].as_str().unwrap();
        let ast = parse(source).unwrap_or_else(|error| panic!("{}: {error}", row["name"]));
        let mut output = Vec::new();
        ast.write_json(&mut output, false).unwrap();
        let ast: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(
            ast["statements"].as_array().unwrap().len(),
            1,
            "{}",
            row["name"]
        );
        let declaration = &ast["statements"][0]["declarationList"]["declarations"][0];
        assert_eq!(
            declaration["name"]["escapedText"], row["nameText"],
            "{}",
            row["name"]
        );
        assert_eq!(
            declaration["initializer"]["text"], row["literalText"],
            "{}",
            row["name"]
        );
    }
}

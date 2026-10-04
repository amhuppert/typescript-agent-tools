use super::*;

impl<'src> Parser<'src> {
    pub(super) fn jsx_element(&mut self) -> Result<NodeId> {
        self.nest(|p| {
            let start = p.pos();
            p.expect("<")?;
            let fragment = p.at(">");
            let mut opening_fields = vec![];
            let tag_name = if fragment {
                None
            } else {
                let tag = p.jsx_name(true)?;
                opening_fields.push(("tagName", tag.into()));
                if p.at("<") {
                    opening_fields.push(("typeArguments", p.type_arguments()?.into()));
                }
                Some(tag)
            };
            if !fragment {
                let attributes_start = p.pos();
                let mut properties = vec![];
                while !p.at(">") && !p.at("/") {
                    let attribute_start = p.pos();
                    if p.eat("{")? {
                        p.expect("...")?;
                        let expression = p.assignment_expression()?;
                        p.expect("}")?;
                        properties.push(p.finish(
                            "JsxSpreadAttribute",
                            attribute_start,
                            fields![("expression", expression.into())],
                        ));
                    } else {
                        let name = p.jsx_name(false)?;
                        let mut fields = fields![("name", name.into())];
                        if p.eat("=")? {
                            let initializer = if p.current.kind == Kind::String {
                                // JSX strings use entities; backslashes are ordinary characters.
                                let token = p.current;
                                let quote = p.lexer.src.as_bytes()[token.start];
                                let remainder = &p.lexer.src[token.start + 1..];
                                let end = remainder
                                    .find(quote as char)
                                    .map(|i| token.start + 2 + i)
                                    .ok_or_else(|| p.error("Unterminated JSX attribute string"))?;
                                let text = &p.lexer.src[token.start + 1..end - 1];
                                p.reset_at(end);
                                p.make(
                                    "StringLiteral",
                                    token.full_start,
                                    end,
                                    fields![
                                        ("text", text.into()),
                                        ("hasExtendedUnicodeEscape", false.into()),
                                    ],
                                )
                            } else if p.at("{") {
                                p.jsx_expression(false)?
                            } else if p.at("<") {
                                p.jsx_element()?
                            } else {
                                return Err(p.error("Expected JSX attribute initializer"));
                            };
                            fields.push(("initializer", initializer.into()));
                        }
                        properties.push(p.finish("JsxAttribute", attribute_start, fields));
                    }
                }
                let attributes = p.finish(
                    "JsxAttributes",
                    attributes_start,
                    fields![("properties", properties.into())],
                );
                opening_fields.push(("attributes", attributes.into()));
            }
            if p.eat("/")? {
                p.expect(">")?;
                return Ok(p.finish("JsxSelfClosingElement", start, opening_fields));
            }
            if !p.at(">") {
                return Err(p.error("Expected '>' in JSX opening tag"));
            }
            let content_start = p.current.end;
            p.end = content_start;
            let opening = p.finish(
                if fragment {
                    "JsxOpeningFragment"
                } else {
                    "JsxOpeningElement"
                },
                start,
                opening_fields,
            );
            let children = p.jsx_children(content_start)?;
            let closing_start = p.current.start;
            p.expect("<")?;
            p.expect("/")?;
            let mut closing_fields = vec![];
            if let Some(tag) = tag_name {
                let closing_tag = p.jsx_name(true)?;
                let opening_text = p.jsx_tag_text(tag);
                let closing_text = p.jsx_tag_text(closing_tag);
                if opening_text != closing_text {
                    return Err(p.error(format!("Expected closing JSX tag for {opening_text}")));
                }
                closing_fields.push(("tagName", closing_tag.into()));
            }
            p.expect(">")?;
            let closing = p.finish(
                if fragment {
                    "JsxClosingFragment"
                } else {
                    "JsxClosingElement"
                },
                closing_start,
                closing_fields,
            );
            Ok(p.finish(
                if fragment {
                    "JsxFragment"
                } else {
                    "JsxElement"
                },
                start,
                fields![
                    (
                        if fragment {
                            "openingFragment"
                        } else {
                            "openingElement"
                        },
                        opening.into(),
                    ),
                    ("children", children.into()),
                    (
                        if fragment {
                            "closingFragment"
                        } else {
                            "closingElement"
                        },
                        closing.into(),
                    ),
                ],
            ))
        })
    }
    fn reset_at(&mut self, offset: usize) {
        self.end = offset;
        self.lexer.pos = offset;
        self.current = self.lexer.next();
    }
    fn jsx_children(&mut self, mut position: usize) -> Result<Vec<NodeId>> {
        let mut children = vec![];
        loop {
            let rest = &self.lexer.src[position..];
            let length = rest.find(['<', '{']).unwrap_or(rest.len());
            if length > 0 {
                let text = &rest[..length];
                if let Some(offset) = text.find(['>', '}']) {
                    return Err(Diagnostic {
                        message: "Escape '>' and '}' in JSX text with an entity or expression"
                            .into(),
                        span: Span {
                            start: position + offset,
                            end: position + offset + 1,
                        },
                    });
                }
                children.push(self.make(
                    "JsxText",
                    position,
                    position + length,
                    fields![
                        ("text", text.into()),
                        (
                            "containsOnlyTriviaWhiteSpaces",
                            (text.chars().any(crate::lexer::line_break)
                                && text.chars().all(crate::lexer::whitespace)).into(),
                        ),
                    ],
                ));
                position += length;
            }
            self.reset_at(position);
            if self.lexer.src[position..].starts_with("</") {
                break;
            }
            if self.current.kind == Kind::Eof {
                return Err(self.error("Unterminated JSX element"));
            }
            if self.at("<") {
                children.push(self.jsx_element()?);
            } else {
                children.push(self.jsx_expression(true)?);
            }
            position = self.end;
        }
        Ok(children)
    }
    fn jsx_expression(&mut self, allow_spread: bool) -> Result<NodeId> {
        let start = self.pos();
        self.expect("{")?;
        let mut fields: Fields<'src> = Fields::new();
        if allow_spread {
            self.optional_token("...", "dotDotDotToken", &mut fields)?;
        }
        if !self.at("}") {
            fields.push(("expression", self.expression()?.into()));
        }
        self.expect("}")?;
        Ok(self.finish("JsxExpression", start, fields))
    }
    fn jsx_tag_text(&self, id: NodeId) -> String {
        let mut id = id;
        let mut suffixes = Vec::new();
        let mut result;
        loop {
            let node = &self.nodes[id.0];
            if node.kind == "ThisKeyword" {
                result = "this".into();
                break;
            } else if node.kind == "Identifier" {
                let Some(Value::Text(text)) = node.field("escapedText") else {
                    unreachable!()
                };
                result = text.to_string();
                break;
            } else {
                let left_key = if node.kind == "JsxNamespacedName" {
                    "namespace"
                } else {
                    "expression"
                };
                let Some(Value::Node(left)) = node.field(left_key) else {
                    unreachable!()
                };
                let Some(Value::Node(right)) = node.field("name") else {
                    unreachable!()
                };
                let Some(Value::Text(text)) = self.nodes[right.0].field("escapedText") else {
                    unreachable!()
                };
                suffixes.push((
                    if node.kind == "JsxNamespacedName" {
                        ':'
                    } else {
                        '.'
                    },
                    text,
                ));
                id = *left;
            }
        }
        for (separator, suffix) in suffixes.into_iter().rev() {
            result.push(separator);
            result.push_str(suffix);
        }
        result
    }
    fn jsx_name(&mut self, dotted: bool) -> Result<NodeId> {
        let start = self.pos();
        let mut name = self.jsx_identifier()?;
        if self.eat(":")? {
            let right = self.jsx_identifier()?;
            return Ok(self.finish(
                "JsxNamespacedName",
                start,
                fields![("namespace", name.into()), ("name", right.into())],
            ));
        }
        if dotted
            && matches!(self.nodes[name.0].field("escapedText"), Some(Value::Text(text)) if text == "this")
        {
            self.nodes[name.0].kind = "ThisKeyword";
            self.nodes[name.0].fields.clear();
        }
        while dotted && self.eat(".")? {
            let right = self.jsx_identifier()?;
            name = self.finish(
                "PropertyAccessExpression",
                start,
                fields![("expression", name.into()), ("name", right.into())],
            );
        }
        Ok(name)
    }
    fn jsx_identifier(&mut self) -> Result<NodeId> {
        let start = self.pos();
        let actual_start = self.current.start;
        if self.current.kind != Kind::Word {
            return Err(self.error("Expected JSX name"));
        }
        if self.current.raw.contains('\\') {
            return Err(self.error("Unicode escapes are not allowed in JSX names"));
        }
        let mut end = actual_start;
        for c in self.lexer.src[actual_start..].chars() {
            if crate::lexer::id_continue(c) || c == '-' {
                end += c.len_utf8();
            } else {
                break;
            }
        }
        let text = &self.lexer.src[actual_start..end];
        self.reset_at(end);
        let escaped = if text.starts_with("__") {
            Value::from(format!("_{text}"))
        } else {
            text.into()
        };
        Ok(self.make("Identifier", start, end, fields![("escapedText", escaped)]))
    }
}

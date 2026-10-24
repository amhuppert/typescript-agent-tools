//! Native JSON binder symbols share the ordinary Store identities and tables.
use super::{Store, SymbolDeclaration, ValueDeclaration, set_value_declaration};
use crate::Span;
use crate::project::binding::ids::{ModuleId, SymbolId};
use crate::project::binding::link::flags;
use crate::project::binding::member::MemberKey;
use crate::project::binding::ordering::NativeSymbolName;
use crate::project::binding::{JsonDeclaration, JsonFacts};
use crate::project::sources::FileId;
use std::collections::HashMap;

impl Store {
    /// Preserve raw JSON node.Symbol identities for native `initializeChecker`;
    /// the normal projection readers below may follow find after merging and
    /// cannot supply this snapshot later.
    pub(in crate::project::binding) fn snapshot_json_symbols(
        &self,
    ) -> crate::project::binding::raw::RawJsonSymbols {
        crate::project::binding::raw::RawJsonSymbols {
            nodes: self
                .json_symbols
                .iter()
                .map(|(&node, &symbol)| (node, self.header_for_symbol(symbol)))
                .collect(),
        }
    }

    /// `bindSourceFileIfExternalModule` creates JSON's distinct export=
    /// Property with the SourceFile ValueDeclaration. `bindAnonymousDeclaration`
    /// and `declareSymbolAndAddToSymbolTable` retain each object and property
    /// node; duplicate property declarations share their actual binder symbol.
    pub(in crate::project::binding) fn bind_json_symbols(
        &mut self,
        module: ModuleId,
        file: FileId,
        span: Span,
        facts: &JsonFacts,
    ) {
        let module_symbol = self.module(module).symbol;
        self.json_symbols
            .insert((file, JsonDeclaration::SourceFile), module_symbol);
        let export = self.new_symbol(flags::PROPERTY, None);
        self.set_native_name(
            export,
            NativeSymbolName::Bytes(b"export=".as_slice().into()),
        );
        self.set_symbol_parent(export, module_symbol);
        self.add_source_file_declaration(export, file, span);
        let bound_table = self.binding_symbol_table(self.declaration_result(module_symbol), true);
        let table = bound_table.projection;
        self.hold(
            table,
            "export=",
            crate::project::binding::meaning::VALUE,
            export,
        );
        self.hold_member(table, &MemberKey::Text("export=".into()), export);
        self.bind_table_symbol(bound_table, self.header_for_symbol(export));
        for value in facts.object_values() {
            let declaration = JsonDeclaration::Value(value);
            let symbol = self.new_symbol(flags::OBJECT_LITERAL, None);
            self.set_native_name(
                symbol,
                NativeSymbolName::Bytes(b"\xfeobject".as_slice().into()),
            );
            self.add_json_declaration(symbol, file, declaration, span, facts);
            self.json_symbols.insert((file, declaration), symbol);
        }
        let mut by_key = HashMap::new();
        for property in facts.reference_keys() {
            let parent = self.json_symbols[&(file, JsonDeclaration::Value(property.object))];
            let key = (property.object, property.key);
            let symbol = if let Some(&symbol) = by_key.get(&key) {
                symbol
            } else {
                let symbol = self.new_symbol(flags::PROPERTY, None);
                let name = match property.key.as_str() {
                    Some(name) => NativeSymbolName::Bytes(name.as_bytes().into()),
                    None => NativeSymbolName::Unavailable(
                        "JSON property name contains an unpaired UTF-16 surrogate",
                    ),
                };
                self.set_native_name(symbol, name);
                self.set_symbol_parent(symbol, parent);
                if let Some(name) = property.key.as_str() {
                    let table = self.binding_symbol_table(self.declaration_result(parent), false);
                    self.hold_member(table.projection, &MemberKey::Text(name.into()), symbol);
                    self.bind_table_symbol(table, self.header_for_symbol(symbol));
                }
                by_key.insert(key, symbol);
                symbol
            };
            let declaration = JsonDeclaration::Property(property.id);
            self.add_json_declaration(symbol, file, declaration, span, facts);
            self.json_symbols.insert((file, declaration), symbol);
        }
    }

    /// `addDeclarationToSymbol`: both ObjectLiteral and Property are in native
    /// SymbolFlagsValue. Their actual ValueDeclaration must survive independently
    /// of getTypeOfSymbol's narrower variable/property value-dispatch families.
    fn add_json_declaration(
        &mut self,
        symbol: SymbolId,
        file: FileId,
        declaration: JsonDeclaration,
        source_span: Span,
        facts: &JsonFacts,
    ) {
        self.write();
        let (kind, span) = facts.declaration_facts(declaration, source_span);
        let declaration = SymbolDeclaration::Json {
            file,
            declaration,
            kind,
            span,
        };
        let value = ValueDeclaration { declaration, kind };
        if let Some(header) = self.raw_projection_header(symbol) {
            if let Some(native) = &mut self.native {
                native.add_declaration(header, declaration, Some(value));
            }
        }
        self.symbols[symbol.index()].declarations.push(declaration);
        set_value_declaration(&mut self.symbols[symbol.index()].value_declaration, value);
    }
}

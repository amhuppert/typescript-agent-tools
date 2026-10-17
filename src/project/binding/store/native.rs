//! Actual `ast.Symbol` headers and native symbol tables.
//!
//! These identities never follow the lexical union-find. A binder declaration
//! may contribute several lexical meanings to one header; `cloneSymbol` creates
//! another header even when navigation treats both symbols as one group.

use super::{
    AliasRef, IntrinsicSymbol, SymbolDeclaration, ValueDeclaration, set_value_declaration,
};
use crate::hash::FastMap;
use crate::project::binding::ids::{NativeHeaderId, NativeTableId, SymbolId};
use crate::project::binding::instances::Instance;
use crate::project::binding::link::flags;
use crate::project::binding::ordering::{NativePropertyKey, NativeSymbolName};
use crate::project::binding::semantic::TypeBindingFailure;
use smallvec::SmallVec;
use std::collections::HashMap;

/// The fields copied or mutated by native binder/checker symbol operations.
/// `projection` is an association to navigation metadata, never the identity
/// of this header and never a source for reconstructing native fields.
#[derive(Debug, Clone)]
pub(in crate::project::binding) struct NativeHeader {
    pub(in crate::project::binding) flags: u32,
    pub(in crate::project::binding) transient: bool,
    /// Native Assignment is independent from navigation's compact flags.
    /// Current retained TS declarations do not produce it; checker merges
    /// and alias eligibility still preserve the actual native decision.
    pub(in crate::project::binding) assignment: bool,
    pub(in crate::project::binding) export_star: bool,
    pub(in crate::project::binding) intrinsic: Option<IntrinsicSymbol>,
    pub(in crate::project::binding) name: Option<NativeSymbolName>,
    pub(in crate::project::binding) declarations: SmallVec<[SymbolDeclaration; 1]>,
    pub(in crate::project::binding) value_declaration: Option<ValueDeclaration>,
    pub(in crate::project::binding) parent: Option<NativeHeaderId>,
    pub(in crate::project::binding) export_symbol: Option<NativeHeaderId>,
    pub(in crate::project::binding) members: Option<NativeTableId>,
    pub(in crate::project::binding) exports: Option<NativeTableId>,
    pub(in crate::project::binding) projection: Option<SymbolId>,
    /// Syntax associations for the native late-member worker, independent of
    /// navigation tables. They follow the owning declaration list in merges.
    pub(in crate::project::binding) late_members: Vec<NativeHeaderId>,
    not_const_enum_only: bool,
}

type NativeTable = super::super::NativeSymbolTable<NativeHeaderId>;

/// Its maps are looked up, never iterated (`crate::hash::FastMap`).
#[derive(Debug, Default)]
pub(in crate::project::binding) struct NativeSymbols {
    headers: Vec<NativeHeader>,
    tables: Vec<NativeTable>,
    declarations: DeclarationSymbols,
    aliases: FastMap<AliasRef, NativeHeaderId>,
    /// The binder table placement is known before its declaration-name
    /// recipe is attached. Publication completes inside the name owner.
    pending_names: FastMap<NativeHeaderId, Vec<NativeTableId>>,
    locals: FastMap<
        (
            crate::project::sources::FileId,
            crate::syntax_kind::SyntaxKind,
            crate::Span,
        ),
        NativeTableId,
    >,
    global_exports: FastMap<crate::project::sources::FileId, NativeTableId>,
    /// Request identities captured before navigation redirects pattern modules.
    requests: FastMap<
        (
            crate::project::sources::FileId,
            crate::project::binding::ids::RequestId,
        ),
        crate::project::binding::raw::RawModuleRequest,
    >,
}

impl NativeSymbols {
    pub(in crate::project::binding) fn with_capacity(headers: usize) -> Self {
        Self {
            headers: Vec::with_capacity(headers),
            ..Self::default()
        }
    }

    /// Binder `bindCommonJSTypeExports`: preserve each raw export's identity
    /// and name on export=, then give that alias its own namespace meaning.
    /// This uses raw Type/Namespace bits, including symbols with Value bits;
    /// the checker's supplemental-export filter is a different question.
    pub(in crate::project::binding) fn bind_common_js_type_exports(
        &mut self,
        module: NativeHeaderId,
    ) -> Result<(), TypeBindingFailure> {
        let Some(exports) = self.header(module).exports else {
            return Ok(());
        };
        let export_equals_key = NativePropertyKey::fixed(b"export=".as_slice(), None);
        let Some(export_equals) = self.lookup(exports, &export_equals_key)? else {
            return Ok(());
        };
        let mut promoted = Vec::new();
        for &symbol in self.entries(exports)?.values() {
            let header = self.header(symbol);
            if header.flags & (flags::TYPE | flags::NAMESPACE) != 0 {
                let name = header
                    .name
                    .as_ref()
                    .ok_or(TypeBindingFailure::NotRetained)?
                    .key()?;
                if name != export_equals_key {
                    promoted.push((name, symbol));
                }
            }
        }
        for (name, symbol) in promoted {
            let table = self.symbol_table(export_equals, true, None);
            self.insert(table, Ok(name), symbol);
            self.header_mut(export_equals).flags |= flags::NAMESPACE_MODULE;
        }
        Ok(())
    }

    /// Published headers and tables never grow: release the capacity their
    /// growth left. Ids and order are unchanged; table contents are not
    /// rehashed.
    pub(in crate::project::binding) fn freeze(&mut self) {
        self.headers.shrink_to_fit();
        self.tables.shrink_to_fit();
        self.declarations.freeze();
    }

    /// Binder `newSymbol` allocates a stable header; checker `newSymbol` also
    /// sets Transient. No declaration, table, parent, or value is invented.
    pub(in crate::project::binding) fn new_symbol(
        &mut self,
        flags: u32,
        transient: bool,
        projection: Option<SymbolId>,
    ) -> NativeHeaderId {
        let flags = crate::project::binding::link::native_symbol_flags(flags);
        let id = NativeHeaderId::new(self.headers.len());
        self.headers.push(NativeHeader {
            flags,
            transient,
            assignment: false,
            export_star: false,
            projection,
            intrinsic: None,
            name: None,
            declarations: SmallVec::new(),
            value_declaration: None,
            parent: None,
            export_symbol: None,
            members: None,
            exports: None,
            late_members: Vec::new(),
            not_const_enum_only: false,
        });
        id
    }

    /// Binder `bindExportDeclaration` declares one ExportStar symbol in the
    /// actual parent's Exports table and appends each full declaration node.
    pub(in crate::project::binding) fn add_export_star(
        &mut self,
        owner: NativeHeaderId,
        file: crate::project::sources::FileId,
        request: crate::project::binding::ids::RequestId,
        span: crate::Span,
    ) -> Result<(), TypeBindingFailure> {
        let table = self.symbol_table(owner, true, None);
        let symbol = self.declare_symbol(
            table,
            NativeSymbolName::Bytes(b"\xfeexport".as_slice().into()),
            0,
            0,
        )?;
        let header = self.header_mut(symbol);
        header.export_star = true;
        header.parent = Some(owner);
        self.add_declaration(
            symbol,
            SymbolDeclaration::ExportStar {
                file,
                request,
                span,
            },
            None,
        );
        Ok(())
    }

    pub(in crate::project::binding) fn set_request(
        &mut self,
        file: crate::project::sources::FileId,
        request: crate::project::binding::ids::RequestId,
        target: crate::project::binding::raw::RawModuleRequest,
    ) {
        self.requests.insert((file, request), target);
    }

    /// Captured program/binder selection before checker pattern augmentation
    /// or getMergedSymbol. Script survives the lexical augmentation tolerance;
    /// None means request selection has not run, rather than ordinary absence.
    pub(in crate::project::binding) fn raw_request(
        &self,
        file: crate::project::sources::FileId,
        request: crate::project::binding::ids::RequestId,
    ) -> Option<crate::project::binding::raw::RawModuleRequest> {
        self.requests.get(&(file, request)).copied()
    }

    pub(in crate::project::binding) fn header(&self, id: NativeHeaderId) -> &NativeHeader {
        &self.headers[id.index()]
    }

    pub(in crate::project::binding) fn header_mut(
        &mut self,
        id: NativeHeaderId,
    ) -> &mut NativeHeader {
        &mut self.headers[id.index()]
    }

    pub(in crate::project::binding) fn set_name(
        &mut self,
        symbol: NativeHeaderId,
        name: NativeSymbolName,
    ) {
        self.headers[symbol.index()].name.get_or_insert(name);
        if let Some(tables) = self.pending_names.remove(&symbol) {
            let key = self.headers[symbol.index()]
                .name
                .as_ref()
                .expect("name just installed")
                .key();
            for table in tables {
                self.tables[table.index()].publish_name();
                self.insert(table, key.clone(), symbol);
            }
        }
    }

    pub(in crate::project::binding) fn hold_symbol(
        &mut self,
        table: NativeTableId,
        symbol: NativeHeaderId,
    ) {
        if let Some(name) = &self.headers[symbol.index()].name {
            self.insert(table, name.key(), symbol);
        } else {
            self.tables[table.index()].hold_name();
            self.pending_names.entry(symbol).or_default().push(table);
        }
    }

    pub(in crate::project::binding) fn new_table(&mut self) -> NativeTableId {
        let id = NativeTableId::new(self.tables.len());
        self.tables.push(NativeTable::default());
        id
    }

    /// `bindContainer`: logical lookup partitions for the same AST node
    /// contribute to one actual Locals table, without uniting their lexical
    /// tables. In particular, parameters and function-body declarations share
    /// the function's native table.
    pub(in crate::project::binding) fn locals_table(
        &mut self,
        owner: (
            crate::project::sources::FileId,
            crate::syntax_kind::SyntaxKind,
            crate::Span,
        ),
        proposed: NativeTableId,
    ) -> NativeTableId {
        *self.locals.entry(owner).or_insert(proposed)
    }

    pub(in crate::project::binding) fn symbol_table(
        &mut self,
        symbol: NativeHeaderId,
        exports: bool,
        proposed: Option<NativeTableId>,
    ) -> NativeTableId {
        let existing = if exports {
            self.header(symbol).exports
        } else {
            self.header(symbol).members
        };
        if let Some(table) = existing {
            return table;
        }
        let table = match proposed {
            Some(table) => table,
            None => self.new_table(),
        };
        if exports {
            self.header_mut(symbol).exports = Some(table);
        } else {
            self.header_mut(symbol).members = Some(table);
        }
        table
    }

    /// `bindNamespaceExportDeclaration` owns SourceFile.GlobalExports,
    /// independently from the module symbol's Exports table.
    pub(in crate::project::binding) fn global_exports(
        &mut self,
        file: crate::project::sources::FileId,
    ) -> NativeTableId {
        if let Some(&table) = self.global_exports.get(&file) {
            return table;
        }
        let table = self.new_table();
        self.global_exports.insert(file, table);
        table
    }

    pub(in crate::project::binding) fn set_value(
        &mut self,
        symbol: NativeHeaderId,
        value: ValueDeclaration,
    ) {
        set_value_declaration(&mut self.header_mut(symbol).value_declaration, value);
    }

    /// `declareSymbolEx` for the retained TypeScript declaration families.
    /// Conflict recovery allocates a new node header without replacing the
    /// table entry. The caller attaches the actual declaration and Parent.
    pub(in crate::project::binding) fn declare_symbol(
        &mut self,
        table: NativeTableId,
        name: NativeSymbolName,
        includes: u32,
        excludes: u32,
    ) -> Result<NativeHeaderId, TypeBindingFailure> {
        self.declare_symbol_with_header(table, name, includes, excludes, None)
    }

    /// Program module discovery can allocate the host before binding its
    /// first declaration. That same header enters its actual local table;
    /// discovery does not allocate an additional declaration symbol.
    pub(in crate::project::binding) fn declare_symbol_with_header(
        &mut self,
        table: NativeTableId,
        name: NativeSymbolName,
        includes: u32,
        excludes: u32,
        allocated: Option<NativeHeaderId>,
    ) -> Result<NativeHeaderId, TypeBindingFailure> {
        let key = name.key();
        let previous = match &key {
            // An unrepresentable different declaration name is retained for
            // semantic table observations; binding a representable key does
            // not enumerate or acquire that unrelated name.
            Ok(key) => self.tables[table.index()].declaration_entry(key)?,
            Err(_) => None,
        };
        let symbol = match previous {
            Some(previous) if self.header(previous).flags & excludes == 0 => previous,
            Some(previous) => {
                let accessor = flags::GETTER | flags::SETTER;
                let old = self.header(previous).flags & accessor;
                if old != 0 && old != includes & accessor {
                    self.header_mut(previous).flags |= accessor;
                }
                self.new_symbol(0, false, None)
            }
            None => match allocated {
                Some(symbol) => symbol,
                None => self.new_symbol(0, false, None),
            },
        };
        self.set_name(symbol, name);
        if previous.is_none() {
            self.insert(table, key, symbol);
        }
        self.header_mut(symbol).flags |= includes;
        Ok(symbol)
    }

    /// `SymbolTable[name]` reads one exact entry. It does not visit headers of
    /// unrelated entries, resolve aliases, or apply `getMergedSymbol`.
    pub(in crate::project::binding) fn lookup(
        &self,
        table: NativeTableId,
        key: &NativePropertyKey,
    ) -> Result<Option<NativeHeaderId>, TypeBindingFailure> {
        self.tables[table.index()].lookup(key)
    }

    pub(in crate::project::binding) fn entries(
        &self,
        table: NativeTableId,
    ) -> Result<&HashMap<NativePropertyKey, NativeHeaderId>, TypeBindingFailure> {
        self.tables[table.index()].entries()
    }

    pub(in crate::project::binding) fn checker_table_copy<T: Copy>(
        &self,
        table: NativeTableId,
        symbol: impl FnMut(NativeHeaderId) -> T,
    ) -> Result<super::super::NativeSymbolTable<T>, TypeBindingFailure> {
        self.tables[table.index()].checker_copy(symbol)
    }

    pub(in crate::project::binding) fn insert(
        &mut self,
        table: NativeTableId,
        key: Result<NativePropertyKey, TypeBindingFailure>,
        symbol: NativeHeaderId,
    ) {
        self.tables[table.index()].insert(key, symbol);
    }

    /// Binder `addDeclarationToSymbol`: append a declaration only once and
    /// record the node's actual Symbol independently of lexical grouping.
    pub(in crate::project::binding) fn add_declaration(
        &mut self,
        symbol: NativeHeaderId,
        declaration: SymbolDeclaration,
        value: Option<ValueDeclaration>,
    ) {
        let header = &mut self.headers[symbol.index()];
        if !header.declarations.contains(&declaration) {
            header.declarations.push(declaration);
        }
        if let Some(value) = value {
            set_value_declaration(&mut header.value_declaration, value);
        }
        if header.flags & flags::CONST_ENUM_ONLY_MODULE != 0
            && header.flags & (flags::FUNCTION | flags::CLASS | flags::REGULAR_ENUM) != 0
        {
            header.flags &= !flags::CONST_ENUM_ONLY_MODULE;
            header.not_const_enum_only = true;
        }
        self.declarations.insert(declaration, symbol);
    }

    /// `bindModuleDeclaration` remembers a previous non-const-enum-only
    /// declaration even when a later namespace body contains only const enums.
    pub(in crate::project::binding) fn module_instance(
        &mut self,
        symbol: NativeHeaderId,
        instance: Instance,
    ) {
        if instance == Instance::NonInstantiated {
            return;
        }
        let header = &mut self.headers[symbol.index()];
        if header.flags & (flags::FUNCTION | flags::CLASS | flags::REGULAR_ENUM) == 0
            && instance == Instance::ConstEnumOnly
            && !header.not_const_enum_only
        {
            header.flags |= flags::CONST_ENUM_ONLY_MODULE;
        } else {
            header.flags &= !flags::CONST_ENUM_ONLY_MODULE;
            header.not_const_enum_only = true;
        }
    }

    pub(in crate::project::binding) fn declaration_symbol(
        &self,
        declaration: SymbolDeclaration,
    ) -> Option<NativeHeaderId> {
        self.declarations.get(declaration)
    }

    pub(in crate::project::binding) fn alias_symbol(
        &self,
        alias: AliasRef,
    ) -> Option<NativeHeaderId> {
        self.aliases.get(&alias).copied()
    }

    pub(in crate::project::binding) fn attach_alias(
        &mut self,
        alias: AliasRef,
        symbol: NativeHeaderId,
    ) {
        self.aliases.insert(alias, symbol);
        self.add_declaration(symbol, SymbolDeclaration::Alias(alias), None);
    }
}

/// The symbol of each declaration, the last one `add_declaration` gave it.
/// Nearly every declaration is a bound one, whose ids are dense in its file,
/// so those occupy a slot of their file's table by id; the rest are keyed.
#[derive(Debug, Default)]
struct DeclarationSymbols {
    /// By file, then by declaration id.
    bound: Vec<Vec<Option<NativeHeaderId>>>,
    other: FastMap<SymbolDeclaration, NativeHeaderId>,
}

impl DeclarationSymbols {
    fn insert(&mut self, declaration: SymbolDeclaration, symbol: NativeHeaderId) {
        let Some((file, id)) = declaration.bound() else {
            self.other.insert(declaration, symbol);
            return;
        };
        if self.bound.len() <= file.index() {
            self.bound.resize_with(file.index() + 1, Vec::new);
        }
        let symbols = &mut self.bound[file.index()];
        if symbols.len() <= id.index() {
            symbols.resize(id.index() + 1, None);
        }
        symbols[id.index()] = Some(symbol);
    }

    fn get(&self, declaration: SymbolDeclaration) -> Option<NativeHeaderId> {
        match declaration.bound() {
            Some((file, id)) => self
                .bound
                .get(file.index())?
                .get(id.index())
                .copied()
                .flatten(),
            None => self.other.get(&declaration).copied(),
        }
    }

    fn freeze(&mut self) {
        for symbols in &mut self.bound {
            symbols.shrink_to_fit();
        }
        self.bound.shrink_to_fit();
    }
}

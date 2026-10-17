//! The tables, symbols, and modules that linking builds and merges.
//!
//! A table entry holds one symbol per meaning plus an optional alias; symbols
//! that merge both ways form one identity, which readers reach with `find`.
//! The methods here are the only writers, and every write marks the store
//! changed: while the global table and module augmentations still merge, a
//! resolution made before a write is discarded before it is read again
//! (`Linker::force`), whichever write it was. Once merging ends the store is
//! frozen, and a write is a bug.

use super::super::sources::FileId;
use super::collect::InterfaceMembers;
use super::ids::{
    AliasId, DeclId, ModuleId, NativeHeaderId, NativeTableId, RequestId, SymbolId, TableId,
    TypeParameterListId,
};
use super::index::Site;
use super::member::MemberKey;
use super::targets::Slots;
use crate::syntax_kind::SyntaxKind;
use smallvec::SmallVec;
use std::collections::HashMap;

mod json;
pub(super) mod native;

/// The global table: script declarations, `declare global` blocks, and UMD
/// globals.
pub(super) const GLOBAL: TableId = TableId::new_const(0);

/// One name in a table: at most one declared symbol per meaning, plus an
/// optional import or export alias. A stored symbol may since have merged
/// into another, so readers canonicalize it with `find`.
#[derive(Debug, Default)]
pub(super) struct Entry {
    pub(super) slots: Slots,
    pub(super) alias: Option<AliasRef>,
    pub(super) member: Option<SymbolId>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ValueDeclaration {
    pub(super) declaration: SymbolDeclaration,
    kind: SyntaxKind,
}

/// One binder result with its independent navigation association. A suppressed
/// duplicate can retain a projected group while the node gets a fresh header.
#[derive(Clone, Copy)]
pub(super) struct DeclaredSymbol {
    pub(super) projection: SymbolId,
    pub(super) header: NativeHeaderId,
}

/// One binder destination: navigation retains its lexical table while the
/// native declaration factory uses the independently owned SymbolTable.
#[derive(Clone, Copy)]
pub(super) struct BindingTable {
    pub(super) projection: TableId,
    pub(super) native: NativeTableId,
}

/// Actual Symbol.Declarations entries, including factories without a DeclId.
/// Native append/merge order is retained here, never reconstructed by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SymbolDeclaration {
    Bound(FileId, DeclId),
    Alias(AliasRef),
    ExportStar {
        file: FileId,
        request: RequestId,
        span: crate::Span,
    },
    Module {
        file: FileId,
        span: crate::Span,
    },
    SourceFile {
        file: FileId,
        span: crate::Span,
    },
    Json {
        file: FileId,
        declaration: super::JsonDeclaration,
        kind: SyntaxKind,
        span: crate::Span,
    },
}
impl SymbolDeclaration {
    pub(super) fn bound(self) -> Option<(FileId, DeclId)> {
        match self {
            Self::Bound(file, id) => Some((file, id)),
            _ => None,
        }
    }
}

/// `SetValueDeclaration`: assignment-only declarations yield to real own
/// declarations; an effective module yields when the next declaration has
/// another kind. Bind and checker merge share this exact precedence owner.
fn set_value_declaration(current: &mut Option<ValueDeclaration>, next: ValueDeclaration) {
    if should_replace_value_declaration(current.map(|old| old.kind), next.kind) {
        *current = Some(next);
    }
}

/// `SetValueDeclaration` precedence shared by retained and checker-created
/// symbols; callers retain their own declaration identity representation.
pub(in crate::project) fn should_replace_value_declaration(
    current: Option<SyntaxKind>,
    next: SyntaxKind,
) -> bool {
    use SyntaxKind::*;
    let assignment = |kind| {
        matches!(
            kind,
            BinaryExpression
                | PropertyAccessExpression
                | ElementAccessExpression
                | Identifier
                | CallExpression
        )
    };
    current.is_none_or(|old| {
        assignment(old) && !assignment(next)
            || old != next && matches!(old, ModuleDeclaration | Identifier)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct AliasRef {
    pub(super) file: FileId,
    pub(super) id: AliasId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntrinsicSymbol {
    Undefined,
    GlobalThis,
}

#[derive(Debug, Default)]
pub(super) struct Symbol {
    pub(super) flags: u32,
    pub(super) intrinsic: Option<IntrinsicSymbol>,
    pub(super) native_name: Option<super::ordering::NativeSymbolName>,
    /// `bindSourceFileAsExternalModule` / ambient module factories create a
    /// quoted native Name. Retain this prefix fact even before a consumer
    /// needs the complete file-name spelling.
    pub(super) quoted_module_name: bool,
    /// The symbol's own declarations: what it declares, and what merges gave
    /// it, one way or both. Compatibility checks read these.
    pub(super) sites: Vec<Site>,
    /// Typed own declaration associations in native bind/merge order. Sites
    /// also contain synthetic lexical attachments and cannot supply this
    /// order. One-way merges copy the native target's declarations while the
    /// source keeps its separate identity; no name-bearing inventory is added.
    pub(super) declarations: SmallVec<[SymbolDeclaration; 1]>,
    pub(super) value_declaration: Option<ValueDeclaration>,
    /// Actual binder ExportSymbol link on the paired local header. Never an
    /// alias target and never inferred from declarations after binding.
    pub(super) export_symbol: Option<SymbolId>,
    pub(super) checker_merged: bool,
    pub(super) late_members: Vec<SymbolId>,
    /// Alias declarations among the symbol's own, as TypeScript's merged
    /// symbol keeps an alias a checker merge met beside declarations. A
    /// group lists what they name as well (`Linker::projected_sites`), but
    /// nothing about those targets is merged into this symbol.
    pub(super) aliases: SmallVec<[AliasRef; 1]>,
    /// Type parameter lists that declare this type parameter.
    pub(super) lists: Vec<(FileId, TypeParameterListId)>,
    pub(super) exports: Option<TableId>,
    pub(super) members: Option<TableId>,
    pub(super) module: Option<ModuleId>,
    /// The tables with an entry that holds this symbol in some meaning (see
    /// `hold`), whose entries gain the meanings a module augmentation merges
    /// into it.
    pub(super) homes: SmallVec<[TableId; 2]>,
}

/// An `export *` of a module: the request it names, which the module it
/// reaches is read through (`Linker::module_stars`), so that it follows the
/// request when an augmentation gives the specifier a module of its own,
/// and whether it is type-only.
#[derive(Debug, Clone, Copy)]
pub(super) struct StarRequest {
    pub(super) file: FileId,
    pub(super) request: RequestId,
    pub(super) type_only: bool,
}

#[derive(Debug)]
pub(super) struct Module {
    pub(super) exports: TableId,
    pub(super) stars: Vec<StarRequest>,
    pub(super) symbol: SymbolId,
    /// A shorthand ambient module (`declare module "x";`), as the first
    /// value declaration of its symbol in merge order decides
    /// (`isShorthandAmbientModuleSymbol`).
    pub(super) shorthand: bool,
    /// A declaration file or ambient module, which may describe CommonJS and
    /// so has a synthetic default export.
    pub(super) declaration: bool,
    /// Separate symbols a module augmentation merged into the module symbol
    /// in meanings it lacks of its own. TypeScript's merged module symbol has
    /// those meanings, so its module object, and every export of a shorthand
    /// module, has them too.
    pub(super) merged: Slots,
    /// The JSON source file whose module this is (`json_export`).
    pub(super) json: Option<FileId>,
    /// The pattern ambient module an augmented specifier's module extends one
    /// way, until it absorbs the pattern's declarations: its exports and
    /// stars complete this module's in the meantime (`Linker::probe_export`).
    pub(super) pattern: Option<ModuleId>,
    /// A checker merge has merged into the module's symbol, as TypeScript's
    /// `getMergedSymbol` then answers a merged symbol for it rather than the
    /// binder's: an augmentation of a specifier that this pattern ambient
    /// module matches then merges into it (`Linker::exact_module`).
    pub(super) merged_into: bool,
}

/// How a checker merge relates its symbols (`mergeSymbol`'s
/// `unidirectional`): both directions make one identity; one way, the source
/// keeps its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Direction {
    Both,
    OneWay,
}

/// What a module request denotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestTarget {
    Module(ModuleId),
    Script,
    /// A tolerated failure: names imported through the request are
    /// unresolved, and an augmentation of it binds nothing.
    Ignored,
}

/// `Store::unite` was asked to make two modules one, which is unsupported.
pub(super) struct TwoModules(pub(super) SymbolId);

#[derive(Debug)]
pub(super) struct Store {
    /// Actual native allocation and tables are independent from the lexical
    /// symbols/tables below. Projection unions must never drain these records.
    native: Option<native::NativeSymbols>,
    symbol_headers: Vec<Option<NativeHeaderId>>,
    table_headers: Vec<Option<NativeTableId>>,
    binding_headers: bool,
    global_this: Option<SymbolId>,
    json_symbols: HashMap<(FileId, super::JsonDeclaration), SymbolId>,
    json_exports: HashMap<FileId, SymbolId>,
    tables: Vec<HashMap<String, Entry>>,
    special_members: HashMap<TableId, HashMap<MemberKey, SymbolId>>,
    symbols: Vec<Symbol>,
    parents: Vec<SymbolId>,
    modules: Vec<Module>,
    /// Parallel to each file's requests.
    requests: Vec<Vec<RequestTarget>>,
    /// By canonical symbol: the members its interface declarations declare,
    /// recorded only for the apparent types of JSON values
    /// (`collect::InterfaceMembers`), so other symbols carry nothing.
    interface_members: HashMap<SymbolId, InterfaceMembers>,
    changed: bool,
    frozen: bool,
}

impl Store {
    /// Actual checker storage. Reading a header never follows either merge
    /// relation; native getMergedSymbol is a separate explicit operation.
    pub(super) fn native_headers(&self) -> &native::NativeSymbols {
        self.native
            .as_ref()
            .expect("binder arena is present before raw publication")
    }
    /// Move the actual binder arena into immutable native inputs. No additional
    /// initialized checker arena survives in lexical Store after this point.
    pub(super) fn take_binding_headers(&mut self) -> native::NativeSymbols {
        assert!(
            !self.binding_headers,
            "raw capture requires completed declarations"
        );
        assert!(
            self.global_this.is_none(),
            "raw capture precedes globalThis creation"
        );
        assert!(!self.frozen, "raw capture precedes completed linking");
        let mut native = self
            .native
            .take()
            .expect("raw binder arena is published once");
        native.freeze();
        native
    }
    pub(super) fn native_headers_mut(&mut self) -> &mut native::NativeSymbols {
        self.write();
        self.native
            .as_mut()
            .expect("binder arena is present before raw publication")
    }
    /// Auxiliary original binder association. Later lexical-only factories
    /// have no semantic identity and cannot masquerade as a binder header.
    pub(super) fn raw_projection_header(&self, symbol: SymbolId) -> Option<NativeHeaderId> {
        self.symbol_headers.get(symbol.index()).copied().flatten()
    }

    pub(super) fn native_table(&self, table: TableId) -> NativeTableId {
        self.table_headers[table.index()].expect("binder table association")
    }

    pub(super) fn binding_table(&self, projection: TableId) -> BindingTable {
        BindingTable {
            projection,
            native: self.table_headers[projection.index()].expect("binder table association"),
        }
    }

    pub(super) fn bind_locals_owner(
        &mut self,
        table: TableId,
        file: FileId,
        kind: SyntaxKind,
        span: crate::Span,
    ) {
        self.write();
        let native = self
            .native
            .as_mut()
            .expect("binder arena before publication")
            .locals_table(
                (file, kind, span),
                self.table_headers[table.index()].expect("binder table association"),
            );
        self.table_headers[table.index()] = Some(native);
    }

    /// `GetSymbolTable(&node.Symbol().Members/Exports)` uses the declaration's
    /// actual header, including a duplicate-recovery header. Lexical grouping
    /// cannot select this table on behalf of the binder.
    pub(super) fn binding_symbol_table(
        &mut self,
        owner: DeclaredSymbol,
        exports: bool,
    ) -> BindingTable {
        let projection = if exports {
            self.export_table(owner.projection)
        } else {
            self.member_table(owner.projection)
        };
        let proposed = (self.header_for_symbol(owner.projection) == owner.header)
            .then_some(self.table_headers[projection.index()].expect("binder table association"));
        let native = self
            .native
            .as_mut()
            .expect("binder arena before publication")
            .symbol_table(owner.header, exports, proposed);
        BindingTable { projection, native }
    }

    pub(super) fn bind_table_symbol(&mut self, table: BindingTable, symbol: NativeHeaderId) {
        self.write();
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .hold_symbol(table.native, symbol);
    }

    pub(super) fn declare_native_symbol(
        &mut self,
        table: BindingTable,
        name: super::ordering::NativeSymbolName,
        includes: u32,
        excludes: u32,
    ) -> Result<NativeHeaderId, super::semantic::TypeBindingFailure> {
        self.write();
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .declare_symbol(
                table.native,
                name,
                super::link::native_symbol_flags(includes),
                excludes,
            )
    }

    pub(super) fn declaration_result(&self, projection: SymbolId) -> DeclaredSymbol {
        DeclaredSymbol {
            projection,
            header: self.symbol_headers[projection.index()].expect("binder symbol association"),
        }
    }

    pub(super) fn header_for_symbol(&self, projection: SymbolId) -> NativeHeaderId {
        self.symbol_headers[projection.index()].expect("binder symbol association")
    }

    pub(super) fn declaration_header(
        &self,
        file: FileId,
        declaration: DeclId,
    ) -> Option<NativeHeaderId> {
        self.native
            .as_ref()
            .expect("binder arena before publication")
            .declaration_symbol(SymbolDeclaration::Bound(file, declaration))
    }

    pub(super) fn native_header_parent(&self, header: NativeHeaderId) -> Option<NativeHeaderId> {
        self.native
            .as_ref()
            .expect("binder arena before publication")
            .header(header)
            .parent
    }

    pub(super) fn attach_projection(&mut self, header: NativeHeaderId, projection: SymbolId) {
        self.write();
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .header_mut(header)
            .projection
            .get_or_insert(projection);
    }

    pub(super) fn add_declaration_flags(&mut self, symbol: DeclaredSymbol, flags: u32) {
        self.write();
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .header_mut(symbol.header)
            .flags |= super::link::native_symbol_flags(flags);
    }

    pub(super) fn set_declaration_name(
        &mut self,
        symbol: DeclaredSymbol,
        name: super::ordering::NativeSymbolName,
    ) {
        self.write();
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .set_name(symbol.header, name.clone());
        let projection = self.find(symbol.projection);
        self.symbols[projection.index()]
            .native_name
            .get_or_insert(name);
    }

    pub(super) fn set_declaration_parent(
        &mut self,
        symbol: DeclaredSymbol,
        parent: DeclaredSymbol,
    ) {
        self.write();
        let header = self
            .native
            .as_mut()
            .expect("binder arena before publication")
            .header_mut(symbol.header);
        if let Some(previous) = header.parent {
            assert_eq!(previous, parent.header, "native binder parent must match");
        } else {
            header.parent = Some(parent.header);
        }
    }

    pub(super) fn set_declaration_export(
        &mut self,
        local: DeclaredSymbol,
        exported: DeclaredSymbol,
    ) {
        self.write();
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .header_mut(local.header)
            .export_symbol = Some(exported.header);
        let projection = self.find(local.projection);
        self.symbols[projection.index()].export_symbol = Some(exported.projection);
    }

    pub(super) fn set_module_instance(
        &mut self,
        symbol: DeclaredSymbol,
        instance: super::instances::Instance,
    ) {
        self.write();
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .module_instance(symbol.header, instance);
    }

    /// Binder alias `declareSymbol` keeps the node's own Alias header even
    /// when navigation represents only its resolution recipe. A rejected
    /// duplicate receives a fresh header and leaves the table entry in place.
    pub(super) fn native_global_exports(&mut self, file: FileId) -> NativeTableId {
        self.write();
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .global_exports(file)
    }

    pub(super) fn bind_module_declaration(
        &mut self,
        table: BindingTable,
        preallocated: Option<SymbolId>,
        name: super::ordering::NativeSymbolName,
        flags: u32,
        file: FileId,
        span: crate::Span,
    ) -> Result<DeclaredSymbol, super::semantic::TypeBindingFailure> {
        self.write();
        let allocated = preallocated.map(|symbol| self.header_for_symbol(symbol));
        let header = self
            .native
            .as_mut()
            .expect("binder arena before publication")
            .declare_symbol_with_header(
                table.native,
                name.clone(),
                flags,
                super::link::native_declaration_excludes(flags),
                allocated,
            )?;
        let projection = match preallocated.or(self
            .native
            .as_ref()
            .expect("binder arena before publication")
            .header(header)
            .projection)
        {
            Some(projection) => projection,
            None => self.new_symbol_projection(flags, None, header),
        };
        let symbol = DeclaredSymbol { projection, header };
        self.attach_projection(header, projection);
        self.set_declaration_name(symbol, name);
        let declaration = SymbolDeclaration::Module { file, span };
        let value = (flags & super::link::flags::VALUE != 0).then_some(ValueDeclaration {
            declaration,
            kind: SyntaxKind::ModuleDeclaration,
        });
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .add_declaration(header, declaration, value);
        let data = &mut self.symbols[projection.index()];
        data.flags |= flags;
        data.declarations.push(declaration);
        if let Some(value) = value {
            set_value_declaration(&mut data.value_declaration, value);
        }
        Ok(symbol)
    }

    pub(super) fn bind_native_alias(
        &mut self,
        table: NativeTableId,
        name: &str,
        alias: AliasRef,
        parent: Option<DeclaredSymbol>,
        kind: SyntaxKind,
    ) -> Result<NativeHeaderId, super::semantic::TypeBindingFailure> {
        self.write();
        if let Some(symbol) = self
            .native
            .as_ref()
            .expect("binder arena before publication")
            .alias_symbol(alias)
        {
            return Ok(symbol);
        }
        let excludes = if kind == SyntaxKind::ExportAssignment {
            u32::MAX
        } else {
            super::link::flags::ALIAS
        };
        let symbol = self
            .native
            .as_mut()
            .expect("binder arena before publication")
            .declare_symbol(
                table,
                super::ordering::NativeSymbolName::Bytes(name.as_bytes().into()),
                super::link::flags::ALIAS,
                excludes,
            )?;
        let parent = parent.map(|parent| parent.header);
        let header = self
            .native
            .as_mut()
            .expect("binder arena before publication")
            .header_mut(symbol);
        if let Some(previous) = header.parent {
            assert_eq!(Some(previous), parent, "native alias parent must match");
        } else {
            header.parent = parent;
        }
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .attach_alias(alias, symbol);
        if kind == SyntaxKind::ExportAssignment && name == "export=" {
            self.native
                .as_mut()
                .expect("binder arena before publication")
                .set_value(
                    symbol,
                    ValueDeclaration {
                        declaration: SymbolDeclaration::Alias(alias),
                        kind,
                    },
                );
        }
        Ok(symbol)
    }

    /// `newSymbol` names are immutable across declaration appends. A native
    /// target retains its name when compatible declarations merge into it.
    pub(super) fn set_native_name(
        &mut self,
        symbol: SymbolId,
        name: super::ordering::NativeSymbolName,
    ) {
        self.write();
        if let Some(header) = self.raw_projection_header(symbol) {
            if let Some(native) = &mut self.native {
                native.set_name(header, name.clone());
            }
        }
        let symbol = self.find(symbol);
        self.symbols[symbol.index()].native_name.get_or_insert(name);
    }
    pub(super) fn new() -> Self {
        let mut native = native::NativeSymbols::default();
        let global = native.new_table();
        Self {
            native: Some(native),
            symbol_headers: Vec::new(),
            table_headers: vec![Some(global)],
            binding_headers: true,
            global_this: None,
            json_symbols: HashMap::new(),
            json_exports: HashMap::new(),
            tables: vec![HashMap::new()],
            special_members: HashMap::new(),
            symbols: Vec::new(),
            parents: Vec::new(),
            modules: Vec::new(),
            requests: Vec::new(),
            interface_members: HashMap::new(),
            changed: false,
            frozen: false,
        }
    }

    /// The members `symbol`'s interface declarations declare, where they are
    /// recorded (`add_interface_members`).
    pub(super) fn interface_members(&self, symbol: SymbolId) -> Option<&InterfaceMembers> {
        self.interface_members.get(&self.find(symbol))
    }

    // Reads

    pub(super) fn table(&self, table: TableId) -> &HashMap<String, Entry> {
        &self.tables[table.index()]
    }

    pub(super) fn entry(&self, table: TableId, name: &str) -> Option<&Entry> {
        self.tables[table.index()].get(name)
    }

    pub(super) fn member(&self, table: TableId, key: &MemberKey) -> Option<SymbolId> {
        let symbol = match key {
            MemberKey::Text(name) => self.entry(table, name)?.member,
            MemberKey::Dynamic => None,
            _ => self.special_members.get(&table)?.get(key).copied(),
        };
        symbol.map(|symbol| self.find(symbol))
    }

    pub(super) fn member_entries(&self, table: TableId) -> Vec<(MemberKey, SymbolId)> {
        self.table(table)
            .iter()
            .filter_map(|(name, entry)| {
                entry
                    .member
                    .map(|symbol| (MemberKey::Text(name.clone()), self.find(symbol)))
            })
            .chain(
                self.special_members
                    .get(&table)
                    .into_iter()
                    .flat_map(|entries| {
                        entries
                            .iter()
                            .map(|(key, &symbol)| (key.clone(), self.find(symbol)))
                    }),
            )
            .collect()
    }

    pub(super) fn hold_member(&mut self, table: TableId, key: &MemberKey, symbol: SymbolId) {
        self.write();
        match key {
            MemberKey::Text(name) => {
                self.tables[table.index()]
                    .entry(name.clone())
                    .or_default()
                    .member = Some(symbol)
            }
            MemberKey::Dynamic => {
                unreachable!("dynamic names retain anonymous declarations outside the named table")
            }
            _ => {
                self.special_members
                    .entry(table)
                    .or_default()
                    .insert(key.clone(), symbol);
            }
        }
    }

    pub(super) fn add_late_member(&mut self, owner: DeclaredSymbol, symbol: SymbolId) {
        self.write();
        let header = self.header_for_symbol(symbol);
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .header_mut(owner.header)
            .late_members
            .push(header);
        let owner = self.find(owner.projection);
        self.symbols[owner.index()].late_members.push(symbol);
    }

    /// The canonical symbol after merges both ways.
    pub(super) fn find(&self, mut id: SymbolId) -> SymbolId {
        while self.parents[id.index()] != id {
            id = self.parents[id.index()];
        }
        id
    }

    /// The canonical symbol's data.
    pub(super) fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[self.find(id).index()]
    }

    /// The canonical type parameter held by a declaration owner's members
    /// (`getSymbolOfDeclaration` for a TypeParameter). Linker's merged-list
    /// checks and retained local-parameter acquisition share this same table
    /// read; one-way merges keep their own parameter identities.
    pub(super) fn type_parameter(&self, owner: SymbolId, name: &str) -> Option<SymbolId> {
        let members = self.symbol(owner).members?;
        self.entry(members, name)?
            .slots
            .get(super::meaning::TYPE)
            .map(|symbol| self.find(symbol))
    }

    /// Every canonical symbol, in creation order.
    pub(super) fn canonical_symbols(&self) -> impl Iterator<Item = (SymbolId, &Symbol)> {
        self.symbols
            .iter()
            .enumerate()
            .filter(|&(index, _)| self.parents[index].index() == index)
            .map(|(index, symbol)| (SymbolId::new(index), symbol))
    }

    pub(super) fn module(&self, module: ModuleId) -> &Module {
        &self.modules[module.index()]
    }

    pub(super) fn request(&self, file: FileId, request: RequestId) -> RequestTarget {
        self.requests[file.index()][request.index()]
    }

    /// Whether anything was written since the last call.
    pub(super) fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// End merging: any later write is a bug.
    pub(super) fn freeze(&mut self) {
        self.frozen = true;
    }

    /// Every write goes through here.
    fn write(&mut self) {
        debug_assert!(!self.frozen, "the store is written after merging ended");
        self.changed = true;
    }

    // Writes

    pub(super) fn new_table(&mut self) -> TableId {
        self.write();
        self.tables.push(HashMap::new());
        self.table_headers
            .push(self.native.as_mut().map(|native| native.new_table()));
        TableId::new(self.tables.len() - 1)
    }

    pub(super) fn new_symbol(&mut self, flags: u32, site: Option<Site>) -> SymbolId {
        self.write();
        let id = SymbolId::new(self.symbols.len());
        let header = self
            .native
            .as_mut()
            .map(|native| native.new_symbol(flags, !self.binding_headers, Some(id)));
        self.new_lexical_symbol(flags, site, header)
    }
    pub(super) fn new_symbol_projection(
        &mut self,
        flags: u32,
        site: Option<Site>,
        header: NativeHeaderId,
    ) -> SymbolId {
        self.new_lexical_symbol(flags, site, Some(header))
    }
    fn new_lexical_symbol(
        &mut self,
        flags: u32,
        site: Option<Site>,
        header: Option<NativeHeaderId>,
    ) -> SymbolId {
        self.write();
        let id = SymbolId::new(self.symbols.len());
        self.symbols.push(Symbol {
            flags,
            sites: site.into_iter().collect(),
            ..Symbol::default()
        });
        self.parents.push(id);
        self.symbol_headers.push(header);
        id
    }

    /// `addUndefinedToGlobalsOrErrorOnRedeclaration` installs the checker's
    /// declarationless Property only if no global entry of any meaning exists.
    /// Existing declarations keep their own symbol even when native reports a
    /// redeclaration diagnostic; they never merge with the intrinsic.
    /// Completed native intrinsic publication has its separate Q1 projection.
    pub(super) fn add_undefined_to_globals(&mut self) {
        if self.entry(GLOBAL, "undefined").is_some() {
            return;
        }
        let symbol = self.new_symbol(super::link::flags::PROPERTY, None);
        self.symbols[symbol.index()].intrinsic = Some(IntrinsicSymbol::Undefined);
        self.set_native_name(
            symbol,
            super::ordering::NativeSymbolName::Bytes(b"undefined".as_slice().into()),
        );
        self.hold(GLOBAL, "undefined", super::meaning::VALUE, symbol);
    }
    pub(super) fn initialize_global_this(&mut self) {
        assert!(self.global_this.is_none(), "globalThis initialized twice");
        let symbol = self.new_symbol(super::link::flags::NAMESPACE, None);
        self.symbols[symbol.index()].intrinsic = Some(IntrinsicSymbol::GlobalThis);
        self.symbols[symbol.index()].exports = Some(GLOBAL);
        self.set_native_name(
            symbol,
            super::ordering::NativeSymbolName::Bytes(b"globalThis".as_slice().into()),
        );
        self.hold(GLOBAL, "globalThis", super::meaning::VALUE, symbol);
        self.hold(GLOBAL, "globalThis", super::meaning::NAMESPACE, symbol);
        self.global_this = Some(symbol);
    }

    /// Exact initialized symbol identity after `getMergedSymbol`. Metadata
    /// availability is a separate question: a missing declaration snapshot
    /// cannot make this actual initialized header stop being globalThis.
    pub(super) fn is_global_this(&self, symbol: SymbolId) -> bool {
        self.global_this
            .is_some_and(|global| self.find(symbol) == global)
    }

    /// A module with its export table and its symbol, whose exports are that
    /// table: namespace declarations merged into the module export through
    /// it.
    pub(super) fn new_module(
        &mut self,
        flags: u32,
        site: Option<Site>,
        shorthand: bool,
        declaration: bool,
    ) -> ModuleId {
        let exports = self.new_table();
        let symbol = self.new_symbol(flags, site);
        let id = ModuleId::new(self.modules.len());
        self.symbols[symbol.index()].module = Some(id);
        self.symbols[symbol.index()].quoted_module_name = true;
        self.symbols[symbol.index()].exports = Some(exports);
        self.modules.push(Module {
            exports,
            stars: Vec::new(),
            symbol,
            shorthand,
            declaration,
            merged: Slots::default(),
            json: None,
            pattern: None,
            merged_into: false,
        });
        id
    }

    pub(super) fn set_json(&mut self, module: ModuleId, file: FileId) {
        self.write();
        self.modules[module.index()].json = Some(file);
    }

    /// Another declaration of a module in the file that first declares it,
    /// as TypeScript's binder adds it to the module's symbol. The first
    /// declaration stays the one that decides whether the module is
    /// shorthand.
    pub(super) fn redeclare_module(&mut self, module: ModuleId, site: Site) {
        self.write();
        let symbol = self.modules[module.index()].symbol;
        self.symbols[symbol.index()].sites.push(site);
    }

    /// A declaration merged into `symbol` by the binder: its flags and site.
    pub(super) fn declare_into(&mut self, symbol: SymbolId, flags: u32, site: Site) {
        self.write();
        let symbol = self.find(symbol);
        let data = &mut self.symbols[symbol.index()];
        data.flags |= flags;
        data.sites.push(site);
    }

    pub(super) fn add_projection_flags(&mut self, symbol: SymbolId, flags: u32) {
        self.write();
        let symbol = self.find(symbol);
        self.symbols[symbol.index()].flags |= flags;
    }

    /// Binder `declareSymbol` records the actual parent, before any checker
    /// canonicalization. Checker `mergeSymbolTable` uses the same association
    /// for its mutable merged parent after an export collision.
    pub(super) fn set_symbol_parent(&mut self, symbol: SymbolId, parent: SymbolId) {
        self.write();
        if self.binding_headers {
            let (header, parent) = (
                self.header_for_symbol(symbol),
                self.header_for_symbol(parent),
            );
            self.native
                .as_mut()
                .expect("binder arena before publication")
                .header_mut(header)
                .parent = Some(parent);
        }
    }

    /// Finish actual binder allocation before checker merging. Projection-only
    /// mutations after this point cannot change the retained native headers.
    pub(super) fn finish_binding_headers(&mut self) {
        self.write();
        self.binding_headers = false;
    }

    pub(super) fn mark_checker_merged(&mut self, symbol: SymbolId) {
        self.write();
        let symbol = self.find(symbol);
        // `mergeSymbol` mutates a transient initialized header in place;
        // unlike a binder symbol, its raw metadata was never replaced.
        if self.symbols[symbol.index()].intrinsic.is_some() {
            return;
        }
        self.symbols[symbol.index()].checker_merged = true;
    }

    /// `addDeclarationToSymbol`: retain the actual own declaration associated
    /// by Linker's existing bind operation. Subsequent merges preserve native
    /// target-then-source order rather than reconstructing it from file order.
    pub(super) fn add_declaration(
        &mut self,
        symbol: SymbolId,
        file: FileId,
        declaration: DeclId,
        kind: SyntaxKind,
        has_value: bool,
    ) {
        self.add_declaration_to_header(
            self.declaration_result(symbol),
            file,
            declaration,
            kind,
            has_value,
        );
    }

    pub(super) fn add_declaration_to_header(
        &mut self,
        declared: DeclaredSymbol,
        file: FileId,
        declaration: DeclId,
        kind: SyntaxKind,
        has_value: bool,
    ) {
        self.write();
        let value = has_value.then_some(ValueDeclaration {
            declaration: SymbolDeclaration::Bound(file, declaration),
            kind,
        });
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .add_declaration(
                declared.header,
                SymbolDeclaration::Bound(file, declaration),
                value,
            );
        let symbol = self.find(declared.projection);
        self.symbols[symbol.index()]
            .declarations
            .push(SymbolDeclaration::Bound(file, declaration));

        if has_value {
            let value = ValueDeclaration {
                declaration: SymbolDeclaration::Bound(file, declaration),
                kind,
            };
            set_value_declaration(&mut self.symbols[symbol.index()].value_declaration, value);
        }
    }

    pub(super) fn add_list(&mut self, symbol: SymbolId, list: (FileId, TypeParameterListId)) {
        self.write();
        let symbol = self.find(symbol);
        self.symbols[symbol.index()].lists.push(list);
    }

    pub(super) fn add_interface_members(&mut self, symbol: SymbolId, members: &InterfaceMembers) {
        self.write();
        let symbol = self.find(symbol);
        let own = self.interface_members.entry(symbol).or_default();
        own.names.extend(members.names.iter().cloned());
        own.inherits |= members.inherits;
    }

    /// An alias declaration a checker merge met beside `symbol`'s
    /// declarations (`Symbol::aliases`).
    pub(super) fn add_alias(&mut self, symbol: SymbolId, alias: AliasRef) {
        self.write();
        let symbol = self.find(symbol);
        let aliases = &mut self.symbols[symbol.index()].aliases;
        if !aliases.contains(&alias) {
            aliases.push(alias);
            let declaration = SymbolDeclaration::Alias(alias);
            self.symbols[symbol.index()].declarations.push(declaration);
        }
    }

    /// bindSourceFileAsExternalModule supplies an actual SourceFile node as
    /// both declaration and ValueDeclaration, even though it has no DeclId.
    pub(super) fn add_source_file_declaration(
        &mut self,
        symbol: SymbolId,
        file: FileId,
        span: crate::Span,
    ) {
        self.write();
        let symbol = self.find(symbol);
        let declaration = SymbolDeclaration::SourceFile { file, span };
        self.symbols[symbol.index()].declarations.push(declaration);
        let value = ValueDeclaration {
            declaration,
            kind: SyntaxKind::SourceFile,
        };
        let header = self.header_for_symbol(symbol);
        self.native
            .as_mut()
            .expect("binder arena before publication")
            .add_declaration(header, declaration, Some(value));
        set_value_declaration(&mut self.symbols[symbol.index()].value_declaration, value);
    }

    /// The member table of `symbol`, created on first use.
    pub(super) fn member_table(&mut self, symbol: SymbolId) -> TableId {
        let symbol = self.find(symbol);
        if let Some(table) = self.symbols[symbol.index()].members {
            return table;
        }
        let table = self.new_table();
        self.symbols[symbol.index()].members = Some(table);
        table
    }

    /// The export table of `symbol`, created on first use.
    pub(super) fn export_table(&mut self, symbol: SymbolId) -> TableId {
        let symbol = self.find(symbol);
        if let Some(table) = self.symbols[symbol.index()].exports {
            return table;
        }
        let table = self.new_table();
        self.symbols[symbol.index()].exports = Some(table);
        table
    }

    /// `bindClassLikeDeclaration` creates a fresh Property|Prototype symbol
    /// before binding the class's children and replaces the native export.
    pub(super) fn class_prototype(&mut self, parent: DeclaredSymbol, flags: u32) -> SymbolId {
        let prototype = self.new_symbol(flags, None);
        self.set_declaration_parent(self.declaration_result(prototype), parent);
        self.set_native_name(
            prototype,
            super::ordering::NativeSymbolName::Bytes(b"prototype".as_slice().into()),
        );
        let table = self.binding_symbol_table(parent, true);
        let exports = table.projection;
        self.tables[exports.index()].insert("prototype".into(), Entry::default());
        self.hold_member(exports, &MemberKey::Text("prototype".into()), prototype);
        self.hold(exports, "prototype", super::meaning::VALUE, prototype);
        self.bind_table_symbol(table, self.header_for_symbol(prototype));
        prototype
    }

    /// Hold `symbol` in `table`'s entry for `name` in one meaning: every write
    /// of a table's symbols goes through here. It records the table among
    /// the symbol's homes, whose entries `Linker::propagate` extends with the
    /// meanings later merges add.
    pub(super) fn hold(&mut self, table: TableId, name: &str, meaning: u8, symbol: SymbolId) {
        self.write();
        match self.tables[table.index()].get_mut(name) {
            Some(entry) => entry.slots.set(meaning, symbol),
            None => {
                let mut entry = Entry::default();
                entry.slots.set(meaning, symbol);
                self.tables[table.index()].insert(name.to_string(), entry);
            }
        }
        let symbol = self.find(symbol);
        let homes = &mut self.symbols[symbol.index()].homes;
        if !homes.contains(&table) {
            homes.push(table);
        }
    }

    /// Set or clear the alias of `table`'s entry for `name`.
    pub(super) fn set_alias(&mut self, table: TableId, name: &str, alias: Option<AliasRef>) {
        self.write();
        match (self.tables[table.index()].get_mut(name), alias) {
            (Some(entry), _) => entry.alias = alias,
            (None, None) => {}
            (None, Some(_)) => {
                let entry = Entry {
                    slots: Slots::default(),
                    alias,
                    member: None,
                };
                self.tables[table.index()].insert(name.to_string(), entry);
            }
        }
    }

    /// Take every entry of `table`, which the binder merges into another.
    pub(super) fn drain(&mut self, table: TableId) -> Vec<(String, Entry)> {
        self.write();
        self.tables[table.index()].drain().collect()
    }

    /// Make `other` part of `target`'s identity, the union every merge of
    /// both directions makes (TypeScript's `recordMergedSymbol`). A module
    /// stays the canonical symbol, so its exports remain the module's own
    /// table. Returns the canonical pair, or nothing when they are one.
    pub(super) fn unite(
        &mut self,
        target: SymbolId,
        other: SymbolId,
    ) -> Result<Option<(SymbolId, SymbolId)>, TwoModules> {
        let (mut target, mut other) = (self.find(target), self.find(other));
        if target == other {
            return Ok(None);
        }
        let module = |symbol: SymbolId| self.symbols[symbol.index()].module.is_some();
        let swap_module = match (module(target), module(other)) {
            (false, true) => true,
            (true, true) => return Err(TwoModules(other)),
            _ => false,
        };
        self.write();

        if swap_module {
            std::mem::swap(&mut target, &mut other);
            // The canonical module ID is a storage constraint. It cannot
            // reverse mergeSymbol's original target/source declaration
            // order when transfer appends the now-canonical pair.
            let original_source = std::mem::take(&mut self.symbols[target.index()].declarations);
            self.symbols[target.index()].declarations =
                std::mem::take(&mut self.symbols[other.index()].declarations);
            self.symbols[other.index()].declarations = original_source;
            let original_source = self.symbols[target.index()].value_declaration.take();
            self.symbols[target.index()].value_declaration =
                self.symbols[other.index()].value_declaration.take();
            self.symbols[other.index()].value_declaration = original_source;
        }
        self.parents[other.index()] = target;
        Ok(Some((target, other)))
    }

    /// What a merge moves (both directions) or copies (one way) from
    /// `source` to `target`: flags, typed declaration associations, declaration sites and alias declarations,
    /// type parameter lists, enum and interface members, and, both ways, the tables that
    /// hold it. Its member and export tables, returned, merge by the caller's
    /// rule: the binder's for declarations of one table
    /// (`Linker::merge_symbols`), the checker's otherwise (`run_merges`).
    pub(super) fn transfer(
        &mut self,
        target: SymbolId,
        source: SymbolId,
        direction: Direction,
    ) -> (Option<TableId>, Option<TableId>) {
        self.write();

        let moved = match direction {
            Direction::Both => std::mem::take(&mut self.symbols[source.index()]),
            Direction::OneWay => {
                let from = &self.symbols[source.index()];
                Symbol {
                    flags: from.flags,
                    quoted_module_name: from.quoted_module_name,
                    sites: from.sites.clone(),
                    declarations: from.declarations.clone(),
                    value_declaration: from.value_declaration,
                    checker_merged: from.checker_merged,
                    late_members: from.late_members.clone(),
                    aliases: from.aliases.clone(),
                    lists: from.lists.clone(),
                    exports: from.exports,
                    members: from.members,
                    ..Symbol::default()
                }
            }
        };
        let members = match direction {
            Direction::Both => self.interface_members.remove(&source),
            Direction::OneWay => self.interface_members.get(&source).cloned(),
        };
        if let Some(members) = members {
            let to = self.interface_members.entry(target).or_default();
            to.names.extend(members.names);
            to.inherits |= members.inherits;
        }
        let to = &mut self.symbols[target.index()];
        to.flags |= moved.flags;
        to.sites.extend(moved.sites);
        to.declarations.extend(moved.declarations);
        if let Some(value) = moved.value_declaration {
            set_value_declaration(&mut to.value_declaration, value);
        }
        // cloneSymbol retains the target's Parent, including a real nil.
        if to.intrinsic.is_none() {
            to.checker_merged |= moved.checker_merged;
        }
        to.late_members.extend(moved.late_members);
        for alias in moved.aliases {
            if !to.aliases.contains(&alias) {
                to.aliases.push(alias);
            }
        }
        to.lists.extend(moved.lists);
        for table in moved.homes {
            if !to.homes.contains(&table) {
                to.homes.push(table);
            }
        }
        (moved.members, moved.exports)
    }

    pub(super) fn add_stars(&mut self, module: ModuleId, stars: &[StarRequest]) {
        self.write();
        self.modules[module.index()].stars.extend_from_slice(stars);
    }

    /// Record that a checker merge merged into `module`'s symbol
    /// (`Module::merged_into`).
    pub(super) fn set_merged_into(&mut self, module: ModuleId) {
        if !self.modules[module.index()].merged_into {
            self.write();
            self.modules[module.index()].merged_into = true;
        }
    }

    /// A meaning a merge gave `module`'s symbol through a separate symbol.
    pub(super) fn set_merged(&mut self, module: ModuleId, meaning: u8, symbol: SymbolId) {
        self.write();
        self.modules[module.index()].merged.set(meaning, symbol);
    }

    pub(super) fn set_pattern(&mut self, module: ModuleId, pattern: Option<ModuleId>) {
        self.write();
        self.modules[module.index()].pattern = pattern;
    }

    /// The targets of the next file's requests.
    pub(super) fn push_requests(&mut self, targets: Vec<RequestTarget>) {
        self.write();
        self.requests.push(targets);
    }

    pub(super) fn set_request(&mut self, file: FileId, request: RequestId, target: RequestTarget) {
        self.write();
        self.requests[file.index()][request.index()] = target;
    }
}

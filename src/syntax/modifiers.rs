//! Syntactic modifier bits are separate from node and effective JSDoc flags.

use crate::syntax_kind::SyntaxKind;

pub(crate) const PUBLIC: u32 = 1 << 0;
pub(crate) const ACCESSIBILITY: u32 = PUBLIC | PRIVATE | PROTECTED;
pub(crate) const EXPORT: u32 = 1 << 5;
pub(crate) const ABSTRACT: u32 = 1 << 6;
pub(crate) const AMBIENT: u32 = 1 << 7;
pub(crate) const READONLY: u32 = 1 << 3;
pub(crate) const PRIVATE: u32 = 1 << 1;
pub(crate) const PROTECTED: u32 = 1 << 2;
pub(crate) const STATIC: u32 = 1 << 8;
pub(crate) const ACCESSOR: u32 = 1 << 9;

/// TypeScript `ModifierToFlag` (internal/ast/utilities.go), including all
/// syntactic modifiers. Unknown tokens carry no modifier bit.
pub(super) fn flag(kind: SyntaxKind) -> u32 {
    match kind {
        SyntaxKind::PublicKeyword => PUBLIC,
        SyntaxKind::PrivateKeyword => PRIVATE,
        SyntaxKind::ProtectedKeyword => PROTECTED,
        SyntaxKind::ReadonlyKeyword => READONLY,
        SyntaxKind::OverrideKeyword => 1 << 4,
        SyntaxKind::ExportKeyword => EXPORT,
        SyntaxKind::AbstractKeyword => ABSTRACT,
        SyntaxKind::DeclareKeyword => AMBIENT,
        SyntaxKind::StaticKeyword => STATIC,
        SyntaxKind::AccessorKeyword => ACCESSOR,
        SyntaxKind::AsyncKeyword => 1 << 10,
        SyntaxKind::DefaultKeyword => 1 << 11,
        SyntaxKind::ConstKeyword => 1 << 12,
        SyntaxKind::InKeyword => 1 << 13,
        SyntaxKind::OutKeyword => 1 << 14,
        SyntaxKind::Decorator => 1 << 15,
        _ => 0,
    }
}

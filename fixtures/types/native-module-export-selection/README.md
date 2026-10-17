# Native module export selection

`getExportOfModule` reads `getExportsOfSymbol`, which selects resolved late-container exports before module exports. A require-context namespace import from an ES-format declaration file reaches `resolveESModuleSymbol`'s `module.exports` branch on the actual class/function target. Alias origins distinguish the late static property, direct static property, and module-flag-gated class. Expression reads observe any propagated type-only marker.

These are deliberately scoped checker-demand controls: export= in an ES-format declaration file, and export-star declarations inside named namespaces, are invalid under whole-file semantic checking. The fresh-query oracle records exactly the diagnostics acquired by the selected demand; it does not imply a healthy whole program. Ordinary source-module type-only stars provide the installed-map control beside class/function late workers, whose maps are discarded.

Existing native-late-symbol-tables/owners cases cover ordinary property and qualified access, but do not reach this alias export-selection branch.

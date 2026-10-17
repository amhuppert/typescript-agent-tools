declare module /*#moduleAliasFailure*/"module-alias-failure" { export = MissingValue; }
declare module /*#moduleAliasHealthy*/"module-alias-healthy" { const stable: MissingValue; export = stable; }
declare module /*#moduleAliasSelf*/"module-alias-self" { namespace inner {} export = inner; }

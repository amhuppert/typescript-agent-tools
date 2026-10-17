// This file is added only through go test -overlay to the pinned 7.0.2 source.
// It is evidence instrumentation, never part of the delivered native tool.
package checker

import "github.com/microsoft/typescript-go/internal/ast"

var AgentOracleCounters = map[string]int{}
var AgentOracleFailures = []string{}
var AgentOracleObservations = map[string]map[string]any{}

func AgentOracleCounter(name string, value int) {
	if previous, exists := AgentOracleCounters[name]; !exists || value > previous {
		AgentOracleCounters[name] = value
	}
}

// First witnesses and maxima bound the evidence payload without accumulating a
// trace for every ordinary relation. They are observations, not root provenance.
func AgentOracleObservation(name string, values map[string]any) {
	if _, exists := AgentOracleObservations[name]; !exists {
		AgentOracleObservations[name] = values
	}
}

func AgentOracleRelationFinish(allowance, cacheSize, result int, overflow bool) {
	AgentOracleCounters["relationFinishedCount"]++
	AgentOracleCounter("relationCacheSize", cacheSize)
	if cacheSize > 0 {
		AgentOracleObservation("relationFirstFinish", map[string]any{
			"allowance": allowance, "cacheSize": cacheSize, "result": result, "overflow": overflow,
		})
	}
}

func AgentOracleFailure(name string) {
	for _, previous := range AgentOracleFailures {
		if previous == name {
			return
		}
	}
	AgentOracleFailures = append(AgentOracleFailures, name)
}

// isErrorType also recognizes the compiler's unresolved alias error types.
// Comparing only GetErrorType identity loses that existing distinction.
func (c *Checker) AgentOracleIsErrorType(t *Type) bool {
	return c.isErrorType(t)
}

// Read the checker's already accumulated diagnostics. Checker.GetDiagnostics
// would instead establish a whole-file checking history, which is not requested.
func (c *Checker) AgentOracleCurrentDiagnostics() []int32 {
	result := []int32{}
	for _, diagnostic := range c.diagnostics.GetDiagnostics() {
		result = append(result, diagnostic.Code())
	}
	return result
}

// AgentOracleCombinedMember exposes the operation named in the design. It does
// not infer origins by joining display names or unrelated reference groups.
func (c *Checker) AgentOracleCombinedMember(symbol *ast.Symbol) *ast.Symbol {
	if symbol == nil || symbol.Parent == nil || symbol.Parent.Flags&(ast.SymbolFlagsClass|ast.SymbolFlagsInterface) == 0 {
		return symbol
	}
	kind := MembersOrExportsResolutionKindResolvedMembers
	for _, declaration := range symbol.Declarations {
		if ast.HasStaticModifier(declaration) {
			kind = MembersOrExportsResolutionKindResolvedExports
			break
		}
	}
	if combined := c.getResolvedMembersOrExportsOfSymbol(symbol.Parent, kind)[symbol.Name]; combined != nil {
		return combined
	}
	return symbol
}

// Scratch-only direct observations; native masks, thresholds and identities unchanged.
func (c *Checker) AgentTypeFacts(t *Type) map[string]any {
  result := map[string]any{"all": uint32(c.getTypeFacts(t, TypeFactsAll)), "flags": uint32(t.flags)}
  masks := []struct { name string; mask TypeFacts }{
    {"truthy",TypeFactsTruthy},{"falsy",TypeFactsFalsy},
    {"typeofString",TypeFactsTypeofEQString},{"typeofNumber",TypeFactsTypeofEQNumber},
    {"typeofBigint",TypeFactsTypeofEQBigInt},{"typeofBoolean",TypeFactsTypeofEQBoolean},
    {"typeofSymbol",TypeFactsTypeofEQSymbol},{"typeofObject",TypeFactsTypeofEQObject},
    {"typeofFunction",TypeFactsTypeofEQFunction},{"notNumber",TypeFactsTypeofNENumber},
    {"notUndefined",TypeFactsNEUndefined},{"notNull",TypeFactsNENull},{"notNullable",TypeFactsNEUndefinedOrNull},
    {"isUndefined",TypeFactsIsUndefined},{"isNull",TypeFactsIsNull},
  }
  rows := []map[string]any{}
  for _, entry := range masks {
    filtered := c.getTypeWithFacts(t, entry.mask)
    adjusted := c.getAdjustedTypeWithFacts(t, entry.mask)
    rows = append(rows, map[string]any{"name":entry.name,"mask":uint32(entry.mask),
      "facts":uint32(c.getTypeFacts(t,entry.mask)),
      "filtered":map[string]any{"same":filtered==t,"flags":uint32(filtered.flags),"summary":c.TypeToString(filtered),"errorType":c.isErrorType(filtered)},
      "adjusted":map[string]any{"same":adjusted==t,"flags":uint32(adjusted.flags),"summary":c.TypeToString(adjusted),"errorType":c.isErrorType(adjusted)}})
  }
  result["masks"] = rows
  return result
}

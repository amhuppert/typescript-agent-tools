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


// Disposable native member question controls. No delivered implementation.
func (c *Checker) AgentOracleMemberValue(t *Type, name string) map[string]any {
  properties := c.GetPropertiesOfType(t)
  names := make([]string, len(properties))
  var chosen *ast.Symbol
  for index, property := range properties {
    names[index] = property.Name
    if property.Name == name { chosen = property }
  }
  if chosen == nil { panic("requested native member is absent: " + name) }
  describe := func(t *Type) map[string]any {
    parts := []*Type{t}
    if t.Flags() & TypeFlagsUnion != 0 { parts = t.Types() }
    containsMissing := false
    alternatives := make([]map[string]any, len(parts))
    for index, part := range parts {
      containsMissing = containsMissing || part == c.missingType
      alternatives[index] = map[string]any{"id":part.Id(), "summary":c.TypeToString(part),
        "flags":part.Flags(), "errorType":c.isErrorType(part), "missing":part == c.missingType}
    }
    return map[string]any{"id":t.Id(), "summary":c.TypeToString(t), "flags":t.Flags(),
      "errorType":c.isErrorType(t), "containsMissing":containsMissing, "alternatives":alternatives}
  }
  output := map[string]any{"names":names, "name":chosen.Name, "flags":chosen.Flags,
    "readonly":c.isReadonlySymbol(chosen), "modifierFlags":getDeclarationModifierFlagsFromSymbol(chosen)}
  read := c.getTypeOfSymbol(chosen)
  output["read"] = describe(read)
  output["afterReadDiagnosticCodes"] = c.AgentOracleCurrentDiagnostics()
  output["nonMissingRead"] = describe(c.getNonMissingTypeOfSymbol(chosen))
  output["afterNonMissingReadDiagnosticCodes"] = c.AgentOracleCurrentDiagnostics()
  output["write"] = describe(c.getWriteTypeOfSymbol(chosen))
  output["afterWriteDiagnosticCodes"] = c.AgentOracleCurrentDiagnostics()
  locations := make([]map[string]any, 0, len(chosen.Declarations))
  for _, declaration := range chosen.Declarations {
    name := ast.GetNameOfDeclaration(declaration)
    if name != nil {
      locations = append(locations, map[string]any{"kind":declaration.Kind.String(),
        "pos":name.Pos(), "end":name.End(), "type":describe(c.GetTypeOfSymbolAtLocation(chosen, name))})
    }
  }
  output["declarations"] = locations
  return output
}

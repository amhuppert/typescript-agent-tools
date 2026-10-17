// This file is added only through go test -overlay to the pinned 7.0.2 source.
// It is evidence instrumentation, never part of the delivered native tool.
package checker

import "github.com/microsoft/typescript-go/internal/ast"

var AgentOracleCounters = map[string]uint64{}
var AgentOracleFailures = []string{}

func AgentOracleCounter(name string, value uint64) {
	if value > AgentOracleCounters[name] {
		AgentOracleCounters[name] = value
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

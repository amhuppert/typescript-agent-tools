// This file is added only through go test -overlay to the pinned 7.0.2 source.
// It is evidence instrumentation, never part of the delivered native tool.
package checker

import "github.com/microsoft/typescript-go/internal/ast"

var AgentOracleCounters = map[string]int{}
var AgentOracleFailures = []string{}
var AgentOracleObservations = map[string]map[string]any{}

// Optional zero-demand node-builder trace. Pointer labels are local evidence
// identities; observing them does not allocate native node/symbol IDs.
var AgentOracleInspectDiagnosticCache bool
// Optional expression traces extend the same zero-demand acquisition stream.
// Disabled requests retain the existing type/symbol-only observations.
var AgentOracleInspectExpressionAcquisitions bool
var AgentOracleDisplayEvents = []map[string]any{}
var AgentOracleDisplayAcquisitions = []map[string]any{}
var agentOracleAcquisitionDepth int

// Entry/exit observations inspect only arguments and the already-returned
// native type. They never resolve a symbol, allocate compiler IDs, or demand a
// type. A separate array preserves the existing cache transition protocol.
func (c *Checker) agentOracleDisplayAcquisition(operation string, node *ast.Node, symbol *ast.Symbol) func(*Type) {
	if !AgentOracleInspectDiagnosticCache { return nil }
	depth := agentOracleAcquisitionDepth
	agentOracleAcquisitionDepth++
	row := func(event string, result *Type) map[string]any {
		x := map[string]any{"event": event, "operation": operation, "depth": depth, "cacheEvents": len(AgentOracleDisplayEvents)}
		if node != nil { x["kind"], x["pos"], x["end"] = node.Kind, node.Pos(), node.End() }
		if symbol != nil {
			x["name"] = symbol.Name
			if symbol.ValueDeclaration != nil { x["declarationPos"], x["declarationEnd"] = symbol.ValueDeclaration.Pos(), symbol.ValueDeclaration.End() }
		}
		if result != nil { x["type"], x["flags"], x["errorType"] = result.id, uint32(result.flags), c.isErrorType(result) }
		return x
	}
	AgentOracleDisplayAcquisitions = append(AgentOracleDisplayAcquisitions, row("enter", nil))
	return func(result *Type) {
		agentOracleAcquisitionDepth--
		AgentOracleDisplayAcquisitions = append(AgentOracleDisplayAcquisitions, row("exit", result))
	}
}

var agentOracleDisplayScopes = map[*ast.Node]int{}
var agentOracleDisplaySymbols = map[*ast.Symbol]int{}

func agentOracleDisplayScope(node *ast.Node) int {
	if node == nil { return 0 }
	if id, ok := agentOracleDisplayScopes[node]; ok { return id }
	id := len(agentOracleDisplayScopes) + 1
	agentOracleDisplayScopes[node] = id
	return id
}

func (b *NodeBuilderImpl) agentOracleDiagnosticVisit(event string, t *Type, key CompositeTypeCacheIdentity, cached *SerializedTypeEntry, depth int) {
	if !AgentOracleInspectDiagnosticCache { return }
	ctx := b.ctx
	row := map[string]any{"event": event, "type": t.id, "scope": agentOracleDisplayScope(ctx.enclosingDeclaration),
		"flags": uint32(key.flags), "internalFlags": int32(key.internalFlags), "currentFlags": uint32(ctx.flags),
		"length": ctx.approximateLength, "truncating": ctx.truncating, "reportedDiagnostic": ctx.reportedDiagnostic,
		"encounteredError": ctx.encounteredError, "tracked": len(ctx.trackedSymbols), "depth": depth,
		"visited": ctx.visitedTypes.Has(t.id), "expansion": ctx.maxExpansionDepth}
	if ctx.enclosingDeclaration != nil {
		row["scopeKind"], row["scopePos"], row["scopeEnd"] = ctx.enclosingDeclaration.Kind, ctx.enclosingDeclaration.Pos(), ctx.enclosingDeclaration.End()
	}
	if cached != nil { row["addedLength"], row["cachedTruncating"], row["cachedTracked"] = cached.addedLength, cached.truncating, len(cached.trackedSymbols) }
	AgentOracleDisplayEvents = append(AgentOracleDisplayEvents, row)
}

func agentOracleDiagnosticTrack(context *NodeBuilderContext, symbol *ast.Symbol, enclosing *ast.Node, meaning ast.SymbolFlags, disabled bool) {
	if !AgentOracleInspectDiagnosticCache { return }
	id, ok := agentOracleDisplaySymbols[symbol]
	if !ok { id = len(agentOracleDisplaySymbols) + 1; agentOracleDisplaySymbols[symbol] = id }
	AgentOracleDisplayEvents = append(AgentOracleDisplayEvents, map[string]any{"event": "track", "symbol": id,
		"name": symbol.Name, "symbolFlags": uint32(symbol.Flags), "scope": agentOracleDisplayScope(enclosing),
		"meaning": uint32(meaning), "disabled": disabled, "tracked": len(context.trackedSymbols)})
}

// Direct owner control over real binder-created symbols. It is separate from
// final-query evidence and asks precisely the native comparison operation.
func (c *Checker) AgentOracleCompareSymbols(left, right *ast.Symbol) int {
	return c.compareSymbolsWorker(left, right)
}

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

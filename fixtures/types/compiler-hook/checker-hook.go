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

// AgentOracleConstructionContext repeats getApparentTypeOfContextualType's
// body for one object literal and returns its instantiated contextual type,
// that type's apparent form (unreduced, the input to object-literal
// discrimination) and the final answer. It acquires the contextual type
// once, exactly as the native body does.
func (c *Checker) AgentOracleConstructionContext(node *ast.Node) (instantiated *Type, undiscriminated *Type, discriminated *Type) {
	contextualType := c.getContextualType(node, ContextFlagsNone)
	instantiatedType := c.instantiateContextualType(contextualType, node, ContextFlagsNone)
	if instantiatedType == nil {
		return nil, nil, nil
	}
	apparentType := c.mapTypeEx(instantiatedType, c.AgentOracleContextualApparent, true)
	if apparentType.flags&TypeFlagsUnion != 0 {
		return instantiatedType, apparentType, c.discriminateContextualTypeByObjectMembers(node, apparentType)
	}
	return instantiatedType, apparentType, apparentType
}

// AgentOracleContextualApparent is getApparentTypeOfContextualType's mapper
// for one constituent: a mapped type stays, any other takes getApparentType.
func (c *Checker) AgentOracleContextualApparent(t *Type) *Type {
	if t.objectFlags&ObjectFlagsMapped != 0 {
		return t
	}
	return c.getApparentType(t)
}

// AgentOracleBaseConstraint is getBaseConstraintOfType for an instantiable
// type (a type parameter, `this`, an indexed access), nil otherwise.
func (c *Checker) AgentOracleBaseConstraint(t *Type) *Type {
	if t.flags&TypeFlagsInstantiable == 0 {
		return nil
	}
	return c.getBaseConstraintOfType(t)
}

// AgentOracleCallable: a type with call or construct signatures, which no
// object literal's type has.
func (c *Checker) AgentOracleCallable(t *Type) bool {
	return len(c.getSignaturesOfType(t, SignatureKindCall)) != 0 || len(c.getSignaturesOfType(t, SignatureKindConstruct)) != 0
}

// AgentOracleMemberKey is the key checkObjectLiteral gives a named member: a
// property assignment, shorthand or method whose computed name has a type
// usable as a property name is named by that type (`[+100]` is `100`); an
// accessor, and any other member, keeps its binder name (`get [+100]` is
// `+100`).
func (c *Checker) AgentOracleMemberKey(member *ast.Node) string {
	renamed := ast.IsPropertyAssignment(member) || ast.IsShorthandPropertyAssignment(member) || ast.IsMethodDeclaration(member)
	if name := member.Name(); renamed && name != nil && ast.IsComputedPropertyName(name) {
		if t := c.checkComputedPropertyName(name); isTypeUsableAsPropertyName(t) {
			return getPropertyNameFromType(t)
		}
	}
	return member.Symbol().Name
}

// AgentOracleAdmitsEveryKey: an alternative against which writing any key is
// no excess-property error (hasExcessProperties skips an empty object type
// and the global Object type), or which an index signature lets any key
// address.
func (c *Checker) AgentOracleAdmitsEveryKey(t *Type) bool {
	return len(c.getIndexInfosOfType(t)) != 0 || c.isEmptyObjectType(t) || t == c.globalObjectType
}

// AgentOracleInstanceOf is Q5's per-form construction identity. A
// non-generic target's alternatives are interned identities. A generic
// declaration's alternative is matched through the provenance TypeScript keeps
// on its instantiations: identity when it contains no type variables, a
// reference's target, its own alias symbol, or an anonymous instantiation's
// target (findMatchingTypeReferenceOrTypeAliasReference's two questions plus
// identity and instantiation).
func (c *Checker) AgentOracleInstanceOf(source, alternative *Type, target *ast.Symbol, generic bool) bool {
	if source == alternative {
		return true
	}
	if !generic || !c.couldContainTypeVariables(alternative) {
		return false
	}
	if alternative.objectFlags&ObjectFlagsReference != 0 {
		return source.objectFlags&ObjectFlagsReference != 0 && source.Target() == alternative.Target()
	}
	if alternative.alias != nil {
		return source.alias != nil && source.alias.symbol == alternative.alias.symbol
	}
	if alternative.flags&TypeFlagsObject != 0 && alternative.objectFlags&ObjectFlagsAnonymous != 0 {
		return source.flags&TypeFlagsObject != 0 && source.objectFlags&ObjectFlagsInstantiated != 0 && source.Target() == alternative
	}
	return false
}

// AgentOracleHasInstanceProvenance: whether AgentOracleInstanceOf can
// recognize every instantiation of this generic declaration's alternative.
func (c *Checker) AgentOracleHasInstanceProvenance(alternative *Type, target *ast.Symbol) bool {
	return !c.couldContainTypeVariables(alternative) ||
		alternative.objectFlags&ObjectFlagsReference != 0 ||
		alternative.alias != nil ||
		alternative.flags&TypeFlagsObject != 0 && alternative.objectFlags&ObjectFlagsAnonymous != 0
}

// AgentOracleIsGenericObjectType exposes the native genericity question.
func (c *Checker) AgentOracleIsGenericObjectType(t *Type) bool {
	return c.isGenericObjectType(t)
}

// AgentOracleTargetProperties is getUnmatchedPropertiesWorker's target
// inventory: every property except static private identifiers, with the
// Optional-or-Partial test that decides whether it is required.
func (c *Checker) AgentOracleTargetProperties(t *Type) ([]*ast.Symbol, []bool) {
	properties := []*ast.Symbol{}
	optional := []bool{}
	for _, property := range c.getPropertiesOfType(t) {
		if isStaticPrivateIdentifierProperty(property) {
			continue
		}
		properties = append(properties, property)
		optional = append(optional, property.Flags&ast.SymbolFlagsOptional != 0 || property.CheckFlags&ast.CheckFlagsPartial != 0)
	}
	return properties, optional
}

// AgentOracleSpreadContribution follows getContextualType's parent dispatch
// from an object literal while each step derives the operand's context from
// its parent's (parentheses, `!`, `as const`, conditional arms, `await`, and
// the operands getContextualTypeForBinaryOperand forwards) and reports whether
// that chain reaches an enclosing object literal's spread operand.
func (c *Checker) AgentOracleSpreadContribution(node *ast.Node) bool {
	for {
		parent := node.Parent
		if parent == nil {
			return false
		}
		switch parent.Kind {
		case ast.KindParenthesizedExpression, ast.KindNonNullExpression, ast.KindAwaitExpression:
		case ast.KindTypeAssertionExpression, ast.KindAsExpression:
			if !ast.IsConstAssertion(parent) {
				return false
			}
		case ast.KindConditionalExpression:
			if node == parent.AsConditionalExpression().Condition {
				return false
			}
		case ast.KindBinaryExpression:
			binary := parent.AsBinaryExpression()
			if binary.Type != nil {
				return false
			}
			switch binary.OperatorToken.Kind {
			case ast.KindBarBarToken, ast.KindQuestionQuestionToken:
				if node == binary.Right {
					t := c.getContextualType(parent, ContextFlagsNone)
					if t == nil || c.patternForType[t] != nil {
						return false
					}
				}
			case ast.KindAmpersandAmpersandToken, ast.KindCommaToken:
				if node != binary.Right {
					return false
				}
			default:
				return false
			}
		case ast.KindSpreadAssignment:
			return true
		default:
			return false
		}
		node = parent
	}
}

// AgentOraclePresence reads a key from the checked literal type: any is
// unknown; each constituent (one, or a union's) provides the key as a required
// or optional named property, or not at all, and an absent key with an
// applicable index signature (getApplicableIndexInfoForName) may be present.
// Only agreement on required, or on absent without an index, is certain.
func (c *Checker) AgentOraclePresence(literalType *Type, key string) string {
	if literalType.flags&TypeFlagsAny != 0 {
		return "unknown"
	}
	types := []*Type{literalType}
	if literalType.flags&TypeFlagsUnion != 0 {
		types = literalType.Types()
	}
	present, absent := 0, 0
	for _, constituent := range types {
		property := c.getPropertyOfType(constituent, key)
		switch {
		case property == nil && c.getApplicableIndexInfoForName(constituent, key) == nil:
			absent++
		case property != nil && property.Flags&ast.SymbolFlagsOptional == 0:
			present++
		}
	}
	switch {
	case present == len(types):
		return "present"
	case absent == len(types):
		return "absent"
	default:
		return "unknown"
	}
}

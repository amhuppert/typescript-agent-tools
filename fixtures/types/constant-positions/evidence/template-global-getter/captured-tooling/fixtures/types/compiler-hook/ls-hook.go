// This file is added only through go test -overlay to the pinned 7.0.2 source.
package ls

import (
	"context"
	"slices"

	"github.com/microsoft/typescript-go/internal/ast"
	"github.com/microsoft/typescript-go/internal/checker"
	"github.com/microsoft/typescript-go/internal/collections"
	"github.com/microsoft/typescript-go/internal/compiler"
)

type AgentOracleCandidate struct {
	Node       *ast.Node
	SymbolMode string
}
type AgentOracleCandidateObservation struct {
	Node          *ast.Node
	Meaning       ast.SemanticMeaning
	ValidRaw      bool
	RawTextMatch  bool
	InDomain      bool
	Symbol        *ast.Symbol
	LexicalSymbol *ast.Symbol
	RelatedSymbol *ast.Symbol
	RelatedKind   int
}

var AgentOracleCandidateNodes []AgentOracleCandidate
var AgentOracleCandidateObservations []AgentOracleCandidateObservation
var AgentOraclePreludeNodes []*ast.Node
var AgentOracleSearchRoots []*ast.Symbol
var AgentOracleCandidateDemandNodes []*ast.Node
var AgentOracleSearchSetCount int
var AgentOracleScheduleEnabled bool

// AgentOracleRoleSymbol keeps the compiler's lexical name precedence while
// exposing the product's first modifier anchor for the parameter's member.
// GetSymbolsOfParameterPropertyDeclaration owns that two-symbol distinction.
func AgentOracleRoleSymbol(c *checker.Checker, node *ast.Node, mode string) (*ast.Symbol, *ast.Node) {
	switch mode {
	case "parameter", "parameterProperty":
		parameter := node.Parent
		local, member := c.GetSymbolsOfParameterPropertyDeclaration(parameter, parameter.Name().Text())
		if mode == "parameterProperty" {
			return member, parameter.Name()
		}
		return local, parameter.Name()
	case "shorthand":
		return c.GetShorthandAssignmentValueSymbol(node.Parent), node
	case "":
		return c.GetSymbolAtLocation(node), node
	default:
		panic("unknown oracle member symbol role: " + mode)
	}
}

func AgentOraclePrelude(c *checker.Checker, node *ast.Node) {
	var calls []*ast.Node
	for current := node; current != nil; current = current.Parent {
		if ast.IsCallExpression(current) || ast.IsNewExpression(current) || ast.IsTaggedTemplateExpression(current) {
			calls = append(calls, current)
		}
	}
	for i := len(calls) - 1; i >= 0; i-- {
		c.GetResolvedSignature(calls[i])
		AgentOraclePreludeNodes = append(AgentOraclePreludeNodes, calls[i])
	}
}

// AgentOracleCallRole applies the design's explicit outer-wrapper normalization
// before the pinned IsCallOrNewExpressionTarget/IsTaggedTemplateTag operations.
// Raw operations are recorded separately; SkipOuterExpressions descends into a
// wrapper and does not itself walk from the selected name up to that wrapper.
func AgentOracleCallRole(node *ast.Node) bool {
	target := node
	if ast.IsRightSideOfPropertyAccess(node) || ast.IsArgumentExpressionOfElementAccess(node) {
		target = node.Parent
	}
	for target.Parent != nil && ast.IsOuterExpression(target.Parent, ast.OEKAll) && target.Parent.Expression() == target {
		target = target.Parent
	}
	return ast.IsCallOrNewExpressionTarget(target, true, false) || ast.IsTaggedTemplateTag(target, true, false)
}

// This callback is inserted immediately after populateSearchSymbolSet, inside
// getReferencedSymbolsForSymbol. Its checker is the relationship owner itself.
func agentOracleAfterSearchSet(state *refState, search *refSearch) {
	if !AgentOracleScheduleEnabled {
		return
	}
	AgentOracleSearchSetCount++
	AgentOracleSearchRoots = append(AgentOracleSearchRoots, search.allSearchSymbols...)
	scope := getSymbolScope(search.symbol)
	rawNodes := map[*ast.SourceFile]map[*ast.Node]bool{}
	for _, candidate := range AgentOracleCandidateNodes {
		source := ast.GetSourceFileOfNode(candidate.Node)
		// Decoded inventory normalization retains the actual native symbol
		// scope. The pinned raw scanner starts from a relative strings.Index
		// without adding container.Pos(); preserve that behavior only in the
		// independent raw position observation below.
		inDomain := scope == nil || ast.GetSourceFileOfNode(scope) == source && candidate.Node.Pos() >= scope.Pos() && candidate.Node.End() <= scope.End()
		if rawNodes[source] == nil {
			rawNodes[source] = map[*ast.Node]bool{}
			if scope == nil || ast.GetSourceFileOfNode(scope) == source {
				for _, raw := range getPossibleSymbolReferenceNodes(source, search.text, scope) {
					rawNodes[source][raw] = true
				}
			}
		}
		if !inDomain {
			AgentOracleCandidateObservations = append(AgentOracleCandidateObservations, AgentOracleCandidateObservation{
				Node: candidate.Node, InDomain: false, RawTextMatch: rawNodes[source][candidate.Node],
			})
			continue
		}
		referenceNode := candidate.Node
		if candidate.SymbolMode == "parameter" || candidate.SymbolMode == "parameterProperty" {
			referenceNode = candidate.Node.Parent.Name()
		}
		// The decoded inventory replaces only the raw text/length prefilter.
		// Ask the native shape predicate with its own exact node text before
		// prelude or symbol acquisition, including for newly inventoried literals.
		valid := false
		switch referenceNode.Kind {
		case ast.KindIdentifier, ast.KindPrivateIdentifier, ast.KindStringLiteral, ast.KindNoSubstitutionTemplateLiteral, ast.KindNumericLiteral:
			valid = isValidReferencePosition(referenceNode, referenceNode.Text())
		case ast.KindDefaultKeyword:
			valid = isValidReferencePosition(referenceNode, "default")
		}
		meaning := getMeaningFromLocation(referenceNode)
		if !valid || meaning&state.searchMeaning == 0 {
			AgentOracleCandidateObservations = append(AgentOracleCandidateObservations, AgentOracleCandidateObservation{
				Node: candidate.Node, Meaning: meaning, ValidRaw: isValidReferencePosition(referenceNode, search.text),
				RawTextMatch: rawNodes[source][candidate.Node], InDomain: true,
			})
			continue
		}
		AgentOracleCandidateDemandNodes = append(AgentOracleCandidateDemandNodes, candidate.Node)
		AgentOraclePrelude(state.checker, candidate.Node)
		symbol, referenceNode := AgentOracleRoleSymbol(state.checker, candidate.Node, candidate.SymbolMode)
		var lexical, related *ast.Symbol
		kind := entryKindNode
		if ast.IsShorthandPropertyAssignment(candidate.Node.Parent) {
			lexical = state.checker.GetShorthandAssignmentValueSymbol(candidate.Node.Parent)
		}
		if symbol != nil && meaning&state.searchMeaning != 0 {
			related, kind = state.getRelatedSymbol(search, symbol, referenceNode)
		}
		AgentOracleCandidateObservations = append(AgentOracleCandidateObservations, AgentOracleCandidateObservation{
			candidate.Node, meaning, isValidReferencePosition(referenceNode, search.text), rawNodes[source][candidate.Node], true, symbol, lexical, related, int(kind),
		})
	}
}

// AgentOracleReferences uses the compiler's existing directional search owner,
// with one checker supplied by the harness for seed, roots and candidate work.
func AgentOracleReferences(ctx context.Context, p *compiler.Program, c *checker.Checker, node *ast.Node, candidates []AgentOracleCandidate, normalize bool, symbolMode string) ([]*SymbolAndEntries, *ast.Symbol, *ast.Symbol) {
	AgentOracleScheduleEnabled = normalize
	if normalize {
		AgentOraclePrelude(c, node)
	}
	raw, referenceNode := AgentOracleRoleSymbol(c, node, symbolMode)
	symbol := raw
	if normalize {
		symbol = c.AgentOracleCombinedMember(raw)
	}
	AgentOracleCandidateNodes = slices.Clone(candidates)
	// The request owner already sorts normalized project-relative UTF-16 spans
	// and role ties. Re-sorting absolute UTF-8 names here changes that schedule.
	sourceFiles := p.GetSourceFiles()
	set := collections.NewSetWithSizeHint[string](len(sourceFiles))
	for _, file := range sourceFiles {
		set.Add(file.FileName())
	}
	if symbol == nil {
		return nil, raw, symbol
	}
	return getReferencedSymbolsForSymbol(ctx, p, symbol, referenceNode, sourceFiles, set, c, refOptions{use: referenceUseReferences}), raw, symbol
}

// AgentOracleReferenceRange observes the native final occurrence range on an
// already-produced entry. It does not acquire a symbol or run checker work.
func AgentOracleReferenceRange(entry *ReferenceEntry) (string, int, int) {
    if entry.textRange != nil {
        return entry.fileName, entry.textRange.Pos(), entry.textRange.End()
    }
    source := ast.GetSourceFileOfNode(entry.node)
    span := getRangeOfNode(entry.node, source, nil)
    return source.FileName(), span.Pos(), span.End()
}

// AgentOracleNodeReferenceRange applies the same native range projection to an
// already-captured normalized candidate, including nodes raw text search skips.
func AgentOracleNodeReferenceRange(node *ast.Node) (string, int, int) {
    source := ast.GetSourceFileOfNode(node)
    span := getRangeOfNode(node, source, nil)
    return source.FileName(), span.Pos(), span.End()
}

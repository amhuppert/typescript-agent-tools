// This test is overlaid into internal/ls. Each invocation is a fresh Go process.
package ls_test

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"testing"

	"github.com/microsoft/typescript-go/internal/ast"
	"github.com/microsoft/typescript-go/internal/astnav"
	"github.com/microsoft/typescript-go/internal/bundled"
	"github.com/microsoft/typescript-go/internal/checker"
	"github.com/microsoft/typescript-go/internal/compiler"
	"github.com/microsoft/typescript-go/internal/core"
	"github.com/microsoft/typescript-go/internal/ls"
	"github.com/microsoft/typescript-go/internal/scanner"
	"github.com/microsoft/typescript-go/internal/tsoptions"
	"github.com/microsoft/typescript-go/internal/vfs/vfstest"
)

type oracleInput struct {
	Files               map[string]string
	Config              string
	File                string
	Offset              int
	Operation           string
	Question            string
	Normalize           bool
	SymbolMode          string
	DemandMode          string
	Realpaths           map[string]string
	CaseSensitive       bool
	SourceFiles         []string
	CompilerOptions     map[string]any
	InspectArrayElement bool
	Candidates          []struct {
		File       string
		Offset     int
		SymbolMode string
		NodeKind   int
		NodeStart  int
		NodeEnd    int
	}
	NodeKind  int
	NodeStart int
	NodeEnd   int
}
type oracleSpan struct {
	File  string `json:"file"`
	Start int    `json:"start"`
	End   int    `json:"end"`
	Kind  int    `json:"kind"`
}

type oracleGuardObservation struct {
	Counters        map[string]int            `json:"counters"`
	DemandFailures  []string                  `json:"demandFailures"`
	Observations    map[string]map[string]any `json:"observations"`
	DiagnosticCodes []int32                   `json:"diagnosticCodes"`
}

func guardObservation(c *checker.Checker) oracleGuardObservation {
	counters := map[string]int{}
	for name, maximum := range checker.AgentOracleCounters {
		counters[name] = maximum
	}
	observations := map[string]map[string]any{}
	for name, witness := range checker.AgentOracleObservations {
		copy := map[string]any{}
		for field, value := range witness {
			copy[field] = value
		}
		observations[name] = copy
	}
	return oracleGuardObservation{Counters: counters, DemandFailures: append([]string{}, checker.AgentOracleFailures...),
		Observations: observations, DiagnosticCodes: c.AgentOracleCurrentDiagnostics()}
}

func span(node *ast.Node) oracleSpan {
	source := ast.GetSourceFileOfNode(node)
	return oracleSpan{source.FileName(), scanner.GetTokenPosOfNode(node, source, false), node.End(), int(node.Kind)}
}

// Exact anchors share the pinned GetTokenAtPosition and token-span operations;
// composite names can be ancestors of the first token, never silent fallbacks.
func anchoredNode(source *ast.SourceFile, offset, kind, start, end int) *ast.Node {
	for node := astnav.GetTokenAtPosition(source, offset); node != nil; node = node.Parent {
		if int(node.Kind) == kind && scanner.GetTokenPosOfNode(node, source, false) == start && node.End() == end {
			return node
		}
	}
	return nil
}
func declarations(symbol *ast.Symbol) []oracleSpan {
	result := []oracleSpan{}
	if symbol != nil {
		for _, declaration := range symbol.Declarations {
			node := declaration.Name()
			if node == nil {
				node = declaration
			}
			origin := span(node)
			origin.Kind = int(declaration.Kind)
			result = append(result, origin)
		}
	}
	return result
}

func TestAgentTypeOracle(t *testing.T) {
	bytes, err := os.ReadFile(os.Getenv("AGENT_TYPE_ORACLE_INPUT"))
	if err != nil {
		t.Fatal(err)
	}
	var input oracleInput
	if err := json.Unmarshal(bytes, &input); err != nil {
		t.Fatal(err)
	}
	files := make(map[string]any, len(input.Files)+len(input.Realpaths))
	for name, text := range input.Files {
		files[name] = text
	}
	for name, target := range input.Realpaths {
		files[name] = vfstest.Symlink(target)
	}
	fs := bundled.WrapFS(vfstest.FromMap(files, input.CaseSensitive))
	host := compiler.NewCompilerHost(filepath.Dir(input.Config), fs, bundled.LibPath(), nil, nil)
	config, errors := tsoptions.GetParsedCommandLineOfConfigFile(input.Config, &core.CompilerOptions{}, nil, host, nil)
	if len(errors) != 0 {
		t.Fatalf("configuration rejected: %v", errors)
	}
	p := compiler.NewProgram(compiler.ProgramOptions{Config: config, Host: host})
	fatal := append([]*ast.Diagnostic{}, p.GetConfigFileParsingDiagnostics()...)
	fatal = append(fatal, p.GetProgramDiagnostics()...)
	fatal = append(fatal, p.GetSyntacticDiagnostics(t.Context(), nil)...)
	fatal = append(fatal, p.GetBindDiagnostics(t.Context(), nil)...)
	if len(fatal) != 0 {
		t.Fatalf("replayed project rejected: %v", fatal)
	}
	optionBytes, err := json.Marshal(config.CompilerOptions())
	if err != nil {
		t.Fatal(err)
	}
	var actualOptions map[string]any
	if err := json.Unmarshal(optionBytes, &actualOptions); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(input.CompilerOptions, actualOptions) {
		t.Fatalf("replayed compiler options differ: expected %v, actual %v", input.CompilerOptions, actualOptions)
	}
	actualFiles := []string{}
	for _, file := range p.GetSourceFiles() {
		if !p.IsSourceFileDefaultLibrary(file.Path()) {
			actualFiles = append(actualFiles, file.FileName())
		}
	}
	slices.Sort(actualFiles)
	slices.Sort(input.SourceFiles)
	if !slices.Equal(actualFiles, input.SourceFiles) {
		t.Fatalf("replayed program sources differ: expected %v, actual %v", input.SourceFiles, actualFiles)
	}
	p.BindSourceFiles()
	c, done := p.GetTypeChecker(t.Context())
	defer done()
	source := p.GetSourceFile(input.File)
	if source == nil {
		t.Fatal("selected file is not in compiler program")
	}
	node := anchoredNode(source, input.Offset, input.NodeKind, input.NodeStart, input.NodeEnd)
	if node == nil {
		t.Fatal("selected compiler anchor kind/span was not found")
	}
	output := map[string]any{}
	if input.Operation == "member" {
		var candidates []ls.AgentOracleCandidate
		for _, candidate := range input.Candidates {
			file := p.GetSourceFile(candidate.File)
			if file == nil {
				t.Fatalf("candidate file missing: %s", candidate.File)
			}
			anchor := anchoredNode(file, candidate.Offset, candidate.NodeKind, candidate.NodeStart, candidate.NodeEnd)
			if anchor == nil {
				t.Fatalf("candidate compiler anchor kind/span was not found in %s", candidate.File)
			}
			candidates = append(candidates, ls.AgentOracleCandidate{Node: anchor, SymbolMode: candidate.SymbolMode})
		}
		entries, raw, combined := ls.AgentOracleReferences(t.Context(), p, c, node, candidates, input.Normalize, input.SymbolMode)
		if raw == nil || c.IsUnknownSymbol(raw) {
			t.Fatal("UnresolvedTarget: selected reference seed has no resolved symbol")
		}
		output["rawOrigins"], output["combinedOrigins"] = declarations(raw), declarations(combined)
		if raw != nil {
			output["rawSymbolFlags"] = raw.Flags
		} else {
			output["rawSymbolFlags"] = ast.SymbolFlagsNone
		}
		roots := [][]oracleSpan{}
		if combined != nil {
			for _, root := range c.GetRootSymbols(combined) {
				roots = append(roots, declarations(root))
			}
		}
		output["roots"] = roots
		groups := []map[string]any{}
		calls := []map[string]any{}
		for _, entry := range entries {
			refs := []oracleSpan{}
			for _, reference := range entry.References() {
				if reference.IsNodeEntry() {
					refs = append(refs, span(reference.Node()))
					calls = append(calls, map[string]any{"span": span(reference.Node()),
						"rawCall": ast.IsCallOrNewExpressionTarget(reference.Node(), true, true),
						"rawTag":  ast.IsTaggedTemplateTag(reference.Node(), true, true), "call": ls.AgentOracleCallRole(reference.Node())})
				}
			}
			groups = append(groups, map[string]any{"origins": declarations(entry.DefinitionSymbol()), "occurrences": refs})
		}
		output["groups"] = groups
		output["callObservations"] = calls
		prelude := []oracleSpan{}
		for _, call := range ls.AgentOraclePreludeNodes {
			prelude = append(prelude, span(call))
		}
		output["prelude"] = prelude
		candidateDemands := []oracleSpan{}
		for _, candidate := range ls.AgentOracleCandidateDemandNodes {
			candidateDemands = append(candidateDemands, span(candidate))
		}
		output["candidateDemands"] = candidateDemands
		candidateObservations := []map[string]any{}
		for _, observation := range ls.AgentOracleCandidateObservations {
			flags := ast.SymbolFlagsNone
			if observation.Symbol != nil {
				flags = observation.Symbol.Flags
			}
			row := map[string]any{
				"span": span(observation.Node), "meaning": observation.Meaning,
				"validRaw": observation.ValidRaw, "rawTextMatch": observation.RawTextMatch, "symbolFlags": flags,
				"origins": declarations(observation.Symbol), "lexicalOrigins": declarations(observation.LexicalSymbol),
				"relatedOrigins": declarations(observation.RelatedSymbol), "relatedKind": observation.RelatedKind,
			}
			candidateObservations = append(candidateObservations, row)
		}
		output["candidateObservations"] = candidateObservations
		output["searchSetCount"] = ls.AgentOracleSearchSetCount
		searchRoots := [][]oracleSpan{}
		for _, root := range ls.AgentOracleSearchRoots {
			searchRoots = append(searchRoots, declarations(root))
		}
		output["searchRoots"] = searchRoots
	} else {
		if input.Normalize {
			ls.AgentOraclePrelude(c, node)
		}
		var typ *checker.Type
		symbol := c.GetSymbolAtLocation(node)
		switch input.SymbolMode {
		case "parameter", "parameterProperty", "shorthand":
			symbol, _ = ls.AgentOracleRoleSymbol(c, node, input.SymbolMode)
		case "alias":
			if symbol != nil && symbol.Flags&ast.SymbolFlagsAlias != 0 {
				symbol = c.GetAliasedSymbol(symbol)
			}
		case "":
		default:
			t.Fatalf("unknown selected symbol mode: %s", input.SymbolMode)
		}
		switch input.Question {
		case "declared":
			typ = c.GetDeclaredTypeOfSymbol(symbol)
		case "value":
			switch input.DemandMode {
			case "location":
				typ = c.GetTypeAtLocation(node)
			case "":
				typ = c.GetTypeOfSymbol(symbol)
			default:
				t.Fatalf("unknown selected demand mode: %s", input.DemandMode)
			}
		default:
			typ = c.GetTypeAtLocation(node)
		}
		if typ == nil {
			t.Fatal("compiler returned no type")
		}
		output["kind"], output["symbolMode"] = input.Question, input.SymbolMode
		output["symbolFlags"] = ast.SymbolFlagsNone
		if symbol != nil {
			output["symbolFlags"] = symbol.Flags
		}

        rawType := c.GetTypeOfSymbol(symbol)
        output["loopIdentity"] = map[string]any{"rawId": rawType.Id(), "actualId": typ.Id(), "sameIdentity": rawType == typ}
        output["beforeSummary"] = guardObservation(c)

		if input.InspectArrayElement {
			if !c.IsArrayType(typ) {
				t.Fatal("selected type is not a native array")
			}
			element := c.GetTypeArguments(typ)[0]
			count := 1
			if element.Flags()&checker.TypeFlagsUnion != 0 {
				count = len(element.Types())
			}
			output["arrayElement"] = map[string]any{"errorType": c.AgentOracleIsErrorType(element), "constituents": count}
		}
		output["summary"], output["errorType"] = c.TypeToString(typ), c.AgentOracleIsErrorType(typ)
	}
	guards := guardObservation(c)
	output["counters"], output["demandFailures"] = guards.Counters, guards.DemandFailures
	output["observations"], output["diagnosticCodes"] = guards.Observations, guards.DiagnosticCodes
	bytes, err = json.Marshal(output)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(os.Getenv("AGENT_TYPE_ORACLE_OUTPUT"), bytes, 0600); err != nil {
		t.Fatal(err)
	}
}

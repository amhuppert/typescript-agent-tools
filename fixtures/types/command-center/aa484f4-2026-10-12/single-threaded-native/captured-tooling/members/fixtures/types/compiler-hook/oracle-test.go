// This test is overlaid into internal/ls. Each invocation is a fresh Go process.
package ls_test

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"slices"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/microsoft/typescript-go/internal/ast"
	"github.com/microsoft/typescript-go/internal/astnav"
	"github.com/microsoft/typescript-go/internal/bundled"
	"github.com/microsoft/typescript-go/internal/checker"
	"github.com/microsoft/typescript-go/internal/compiler"
	"github.com/microsoft/typescript-go/internal/core"
	"github.com/microsoft/typescript-go/internal/ls"
	"github.com/microsoft/typescript-go/internal/scanner"
	"github.com/microsoft/typescript-go/internal/tsoptions"
	"github.com/microsoft/typescript-go/internal/vfs"
)

type oracleInput struct {
	Files               map[string]string
	Config              string
	CurrentDirectory    string
	File                string
	Offset              int
	Operation           string
	Question            string
	Normalize           bool
	SymbolMode          string
	DemandMode          string
	FileSystem          oracleFileSystemAnswers
	CaseSensitive       bool
	SourceFiles         []string
	CompilerOptions     map[string]any
	InspectArrayElement bool
	InspectSummaryBytes bool
	InspectDiagnosticCache bool
	InspectExpressionAcquisitions bool
	InspectSymbolDisplay bool
	SymbolDisplayEnclosing bool
	SymbolIDPrefix      int
	SingleThreadBinding bool
	InspectPrivateNames bool
	ComparePrototypeSymbols bool
	ObserveRightPrototypeFirst bool
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

// Replay callbackFS answers, including negative observations. Existence does
// not imply readable contents, and an unobserved path is not an absent path.
// Bundled libraries wrap this filesystem using the pinned embedded sources.
type oracleFileSystemAnswers struct {
	ReadFile             map[string]*string
	FileExists           map[string]bool
	DirectoryExists      map[string]bool
	GetAccessibleEntries map[string]vfs.Entries
	Realpath             map[string]string
}

type oracleFileSystem struct {
	answers oracleFileSystemAnswers
	caseSensitive bool
	missing sync.Once
}

func (fs *oracleFileSystem) require(operation, path string) {
	fs.missing.Do(func() {
		request, err := json.Marshal(map[string]string{"operation": operation, "path": path})
		if err != nil { panic(err) }
		if err := os.WriteFile(os.Getenv("AGENT_TYPE_ORACLE_FS_REQUEST"), request, 0o600); err != nil { panic(err) }
	})
	panic(fmt.Sprintf("compiler hook filesystem needs captured answer: %s %s", operation, path))
}

func (fs *oracleFileSystem) UseCaseSensitiveFileNames() bool { return fs.caseSensitive }
func (fs *oracleFileSystem) FileExists(path string) bool {
	answer, ok := fs.answers.FileExists[path]
	if !ok { fs.require("fileExists", path) }
	return answer
}
func (fs *oracleFileSystem) ReadFile(path string) (string, bool) {
	answer, ok := fs.answers.ReadFile[path]
	if !ok { fs.require("readFile", path) }
	if answer == nil { return "", false }
	// Captures retain original UTF-8 for hashes; callbackFS receives decoded
	// compiler text, matching vfs.decodeBytes' UTF-8 BOM treatment.
	return strings.TrimPrefix(*answer, "\uFEFF"), true
}
func (fs *oracleFileSystem) DirectoryExists(path string) bool {
	answer, ok := fs.answers.DirectoryExists[path]
	if !ok { fs.require("directoryExists", path) }
	return answer
}
func (fs *oracleFileSystem) GetAccessibleEntries(path string) vfs.Entries {
	answer, ok := fs.answers.GetAccessibleEntries[path]
	if !ok { fs.require("getAccessibleEntries", path) }
	// The native API callback does not carry Symlinks. Copy arrays so a caller
	// cannot change the frozen answer through a returned slice.
	return vfs.Entries{Files: slices.Clone(answer.Files), Directories: slices.Clone(answer.Directories)}
}
func (fs *oracleFileSystem) Realpath(path string) string {
	answer, ok := fs.answers.Realpath[path]
	if !ok { fs.require("realpath", path) }
	return answer
}

// These operations have no API callbacks. If a new compiler path needs one,
// the capture contract must be extended explicitly, never delegated to disk.
func (*oracleFileSystem) WriteFile(path, data string) error { panic("compiler hook filesystem: unsupported WriteFile " + path) }
func (*oracleFileSystem) AppendFile(path, data string) error { panic("compiler hook filesystem: unsupported AppendFile " + path) }
func (*oracleFileSystem) Remove(path string) error { panic("compiler hook filesystem: unsupported Remove " + path) }
func (*oracleFileSystem) Chtimes(path string, atime, mtime time.Time) error { panic("compiler hook filesystem: unsupported Chtimes " + path) }
func (*oracleFileSystem) Stat(path string) vfs.FileInfo { panic("compiler hook filesystem: unsupported Stat " + path) }
func (*oracleFileSystem) WalkDir(path string, walk vfs.WalkDirFunc) error { panic("compiler hook filesystem: unsupported WalkDir " + path) }

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
	DisplayEvents []map[string]any            `json:"displayEvents,omitempty"`
	DisplayAcquisitions []map[string]any      `json:"displayAcquisitions,omitempty"`
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
		Observations: observations, DiagnosticCodes: c.AgentOracleCurrentDiagnostics(),
		DisplayEvents: append([]map[string]any(nil), checker.AgentOracleDisplayEvents...),
		DisplayAcquisitions: append([]map[string]any(nil), checker.AgentOracleDisplayAcquisitions...)}
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
	checker.AgentOracleInspectDiagnosticCache = input.InspectDiagnosticCache
	checker.AgentOracleInspectExpressionAcquisitions = input.InspectExpressionAcquisitions
	// A controlled prior process history: allocate unrelated native symbols
	// before constructing this request's fresh program and checker.
	if input.SymbolIDPrefix < 0 || input.SymbolIDPrefix > 10000 {
		t.Fatal("invalid symbol-ID prefix")
	}
	for range input.SymbolIDPrefix {
		ast.GetSymbolId(&ast.Symbol{})
	}
	fs := bundled.WrapFS(&oracleFileSystem{answers: input.FileSystem, caseSensitive: input.CaseSensitive})
	host := compiler.NewCompilerHost(input.CurrentDirectory, fs, bundled.LibPath(), nil, nil)
	config, errors := tsoptions.GetParsedCommandLineOfConfigFile(input.Config, &core.CompilerOptions{}, nil, host, nil)
	if len(errors) != 0 {
		t.Fatalf("configuration rejected: %v", errors)
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
	// The observation stream belongs to this request's one checker. Native's
	// default pool eagerly initializes four checkers, which would mix unrelated
	// initialization events and race the observer maps. Checkers controls only
	// pool size; preserve independent native binder scheduling below. Validate
	// all original options before applying this evidence-only performance option.
	checkerCount := 1
	config.CompilerOptions().Checkers = &checkerCount
	singleThreaded := core.TSUnknown
	if input.SingleThreadBinding { singleThreaded = core.TSTrue }
	p := compiler.NewProgram(compiler.ProgramOptions{Config: config, Host: host, SingleThreaded: singleThreaded})
	fatal := append([]*ast.Diagnostic{}, p.GetConfigFileParsingDiagnostics()...)
	fatal = append(fatal, p.GetProgramDiagnostics()...)
	fatal = append(fatal, p.GetSyntacticDiagnostics(t.Context(), nil)...)
	fatal = append(fatal, p.GetBindDiagnostics(t.Context(), nil)...)
	if len(fatal) != 0 {
		t.Fatalf("replayed project rejected: %v", fatal)
	}
	actualFiles := []string{}
	// GetSourceFiles and the two Program provenance questions are observed on
	// the replay program itself. API input metadata cannot stand in for this.
	programFiles := []map[string]any{}
	for _, file := range p.GetSourceFiles() {
		programFiles = append(programFiles, map[string]any{
			"file": file.FileName(),
			"defaultLibrary": p.IsSourceFileDefaultLibrary(file.Path()),
			"externalLibrary": p.IsSourceFileFromExternalLibrary(file),
			"sourceTextSha256": fmt.Sprintf("%x", sha256.Sum256([]byte(file.Text()))),
		})
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
	output := map[string]any{"checkerCount": checkerCount, "programFiles": programFiles}
	if input.ComparePrototypeSymbols {
		prototypes := []*ast.Symbol{}
		var visit func(*ast.Node) bool
		visit = func(node *ast.Node) bool {
			if ast.IsClassLike(node) && node.Symbol() != nil {
				prototype := node.Symbol().Exports["prototype"]
				if prototype == nil || prototype.Flags&ast.SymbolFlagsPrototype == 0 || len(prototype.Declarations) != 0 {
					t.Fatal("actual class prototype header differs")
				}
				prototypes = append(prototypes, prototype)
			}
			return node.ForEachChild(visit)
		}
		source.ForEachChild(visit)
		if len(prototypes) != 2 || prototypes[0] == prototypes[1] || prototypes[0].Name != prototypes[1].Name {
			t.Fatal("prototype comparison requires distinct actual symbols with equal names")
		}
		left, right := prototypes[0], prototypes[1]
		if input.ObserveRightPrototypeFirst { ast.GetSymbolId(right) }
		output["prototypeComparison"] = map[string]any{
			"forward": c.AgentOracleCompareSymbols(left, right),
			"reverse": c.AgentOracleCompareSymbols(right, left),
			"same": c.AgentOracleCompareSymbols(left, left),
			"name": left.Name,
		}
	}
	if input.InspectPrivateNames {
		// Binding has already allocated these names. Reading their decimal
		// suffixes observes that allocation without acquiring any new IDs.
		rows := []map[string]string{}
		var visit func(*ast.Node) bool
		visit = func(node *ast.Node) bool {
			if ast.IsClassLike(node) && node.Symbol() != nil {
				for name := range node.Symbol().Members {
					if suffix, found := strings.CutPrefix(name, ast.InternalSymbolNamePrefix+"#"); found {
						rows = append(rows, map[string]string{"class": node.Symbol().Name, "suffix": suffix})
					}
				}
			}
			return node.ForEachChild(visit)
		}
		for _, file := range p.GetSourceFiles() {
			file.ForEachChild(visit)
		}
		output["privateNames"] = rows
	}
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
        nativeRangeGroups := []map[string]any{}
		calls := []map[string]any{}
		for _, entry := range entries {
			refs := []oracleSpan{}
            nativeRanges := []oracleSpan{}
			for _, reference := range entry.References() {
				if reference.IsNodeEntry() {
					refs = append(refs, span(reference.Node()))
                    file, start, end := ls.AgentOracleReferenceRange(reference)
                    nativeRanges = append(nativeRanges, oracleSpan{file, start, end, int(reference.Node().Kind)})
					calls = append(calls, map[string]any{"span": span(reference.Node()),
						"rawCall": ast.IsCallOrNewExpressionTarget(reference.Node(), true, true),
						"rawTag":  ast.IsTaggedTemplateTag(reference.Node(), true, true), "call": ls.AgentOracleCallRole(reference.Node())})
				}
			}
			groups = append(groups, map[string]any{"origins": declarations(entry.DefinitionSymbol()), "occurrences": refs})
            nativeRangeGroups = append(nativeRangeGroups, map[string]any{"origins": declarations(entry.DefinitionSymbol()), "occurrences": nativeRanges})
		}
		output["groups"] = groups
        output["nativeRangeGroups"] = nativeRangeGroups
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
			rangeFile, rangeStart, rangeEnd := ls.AgentOracleNodeReferenceRange(observation.Node)
            row := map[string]any{
                "nativeRange": oracleSpan{rangeFile, rangeStart, rangeEnd, int(observation.Node.Kind)},
				"span": span(observation.Node), "meaning": observation.Meaning,
				"validRaw": observation.ValidRaw, "rawTextMatch": observation.RawTextMatch, "inDomain": observation.InDomain, "symbolFlags": flags,
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
			if !input.InspectSymbolDisplay && symbol != nil && symbol.Flags&ast.SymbolFlagsAlias != 0 {
				symbol = c.GetAliasedSymbol(symbol)
			}
		case "":
		default:
			t.Fatalf("unknown selected symbol mode: %s", input.SymbolMode)
		}
		if input.InspectSymbolDisplay {
			if symbol == nil { t.Fatal("selected node has no symbol for SymbolToString") }
			output["beforeSymbolDisplay"] = guardObservation(c)
			if input.SymbolDisplayEnclosing {
				output["symbolDisplayBytes"] = []byte(c.SymbolToStringEx(symbol, node, ast.SymbolFlagsAll, checker.SymbolFormatFlagsDoNotIncludeSymbolChain|checker.SymbolFormatFlagsAllowAnyNodeKind))
				output["defaultResolutionMode"] = p.GetDefaultResolutionModeForFile(source)
				if ast.IsStringLiteralLike(node) {
					resolved := p.GetResolvedModuleFromModuleSpecifier(source, node)
					if resolved.IsResolved() { output["selectedModuleResolvedPath"] = resolved.ResolvedFileName }
				}
				imports := []map[string]any{}
				for _, specifier := range source.Imports() {
					entry := map[string]any{"text": specifier.Text(), "mode": p.GetModeForUsageLocation(source, specifier), "resolvedPath": nil}
					resolved := p.GetResolvedModuleFromModuleSpecifier(source, specifier)
					if resolved.IsResolved() { entry["resolvedPath"] = resolved.ResolvedFileName }
					imports = append(imports, entry)
				}
				output["importResolutionModes"] = imports
			} else {
				output["symbolDisplayBytes"] = []byte(c.SymbolToString(symbol))
			}
			output["symbolDisplayFlags"] = symbol.Flags
		} else {
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
		summary := c.TypeToString(typ)
		output["summary"], output["errorType"] = summary, c.AgentOracleIsErrorType(typ)
		if input.InspectSummaryBytes {
			// Go strings retain a native byte cutoff even when it splits UTF-8.
			// []byte marshals to base64 without JSON's invalid-UTF8 replacement.
			// Observe the existing display result; never serialize a second time.
			output["summaryBytes"] = []byte(summary)
		}
		}
	}
	guards := guardObservation(c)
	output["counters"], output["demandFailures"] = guards.Counters, guards.DemandFailures
	output["observations"], output["diagnosticCodes"] = guards.Observations, guards.DiagnosticCodes
	if input.InspectDiagnosticCache { output["displayEvents"], output["displayAcquisitions"] = guards.DisplayEvents, guards.DisplayAcquisitions }
	bytes, err = json.Marshal(output)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(os.Getenv("AGENT_TYPE_ORACLE_OUTPUT"), bytes, 0600); err != nil {
		t.Fatal(err)
	}
}

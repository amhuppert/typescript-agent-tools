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
	NoTruncation bool
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
	} else if input.Operation == "constructs" {
		output["constructs"] = constructionSites(t, p, c, node, input)
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
        var summary string
        if input.NoTruncation {
            summary = c.TypeToStringEx(typ, nil, checker.TypeFormatFlagsAllowUniqueESSymbolType|checker.TypeFormatFlagsUseAliasDefinedOutsideCurrentScope|checker.TypeFormatFlagsNoTruncation, nil)
        } else {
            summary = c.TypeToString(typ)
        }
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

// constructionSites observes Q5's per-literal facts on the pinned checker,
// independently of the product's enumeration: every object literal in a
// searched file, whether the default universe examines it, the
// assignment-pattern and spread-contribution exclusions,
// getApparentTypeOfContextualType before and after object-literal
// discrimination, the per-form declared-type instance question, and presence
// read from the full checked literal type.
func constructionSites(t *testing.T, p *compiler.Program, c *checker.Checker, node *ast.Node, input oracleInput) map[string]any {
	symbol := c.GetSymbolAtLocation(node)
	if input.SymbolMode == "alias" && symbol != nil && symbol.Flags&ast.SymbolFlagsAlias != 0 {
		symbol = c.GetAliasedSymbol(symbol)
	}
	if symbol == nil || input.Question != "declared" {
		t.Fatal("constructs requires a declared type selection")
	}
	declared := c.GetDeclaredTypeOfSymbol(symbol)
	constituents := []*checker.Type{declared}
	if declared.Flags()&checker.TypeFlagsUnion != 0 {
		constituents = declared.Types()
	}
	alternatives := []*checker.Type{}
	for _, constituent := range constituents {
		if objectLike(constituent) && !c.AgentOracleCallable(constituent) {
			alternatives = append(alternatives, constituent)
		}
	}
	result := map[string]any{"target": c.TypeToString(declared)}
	if len(alternatives) == 0 {
		result["unsupported"] = "no object alternative"
		return result
	}
	generic := len(c.GetLocalTypeParametersOfClassOrInterfaceOrTypeAlias(symbol)) != 0
	alternativeSummaries := []string{}
	for _, alternative := range alternatives {
		if c.AgentOracleIsGenericObjectType(alternative) {
			result["unsupported"] = "generic object type"
			return result
		}
		if generic && !c.AgentOracleHasInstanceProvenance(alternative, symbol) {
			result["unsupported"] = "alternative without instantiation provenance"
			return result
		}
		alternativeSummaries = append(alternativeSummaries, c.TypeToString(alternative))
	}
	result["alternatives"], result["generic"] = alternativeSummaries, generic
	// Q5's default universe, decided from each literal's syntax and the
	// binder's member names: an alternative that admits every key makes every
	// literal examined; otherwise a literal is examined unless
	// inDefaultUniverse rules it out.
	keys := map[string]bool{}
	universe := "target_keys"
	for _, alternative := range alternatives {
		if c.AgentOracleAdmitsEveryKey(alternative) {
			universe = "all"
		}
		for _, property := range c.GetPropertiesOfType(alternative) {
			keys[property.Name] = true
		}
	}
	result["universe"] = universe
	literals := []*ast.Node{}
	for _, file := range p.GetSourceFiles() {
		// A JSON module's literals have no contextual type: never constructions.
		if p.IsSourceFileDefaultLibrary(file.Path()) || p.IsSourceFileFromExternalLibrary(file) || ast.IsJsonSourceFile(file) {
			continue
		}
		var visit func(*ast.Node) bool
		visit = func(current *ast.Node) bool {
			if current.Kind == ast.KindObjectLiteralExpression {
				literals = append(literals, current)
			}
			current.ForEachChild(visit)
			return false
		}
		file.ForEachChild(visit)
	}
	slices.SortStableFunc(literals, func(a, b *ast.Node) int {
		left, right := ast.GetSourceFileOfNode(a), ast.GetSourceFileOfNode(b)
		if order := strings.Compare(left.FileName(), right.FileName()); order != 0 {
			return order
		}
		return scanner.GetTokenPosOfNode(a, left, false) - scanner.GetTokenPosOfNode(b, right, false)
	})
	rows := []map[string]any{}
	for _, literal := range literals {
		row := map[string]any{"span": span(literal)}
		rows = append(rows, row)
		row["examinedByDefault"] = universe == "all" || inDefaultUniverse(c, literal, keys)
		if ast.IsAssignmentTarget(literal) {
			row["excluded"] = "pattern"
			continue
		}
		if c.AgentOracleSpreadContribution(literal) {
			row["excluded"] = "spread"
			continue
		}
		// getContextualType on demand: an argument context resolves its own
		// call, and an annotation needs none, so no checking prelude runs.
		instantiated, undiscriminated, discriminated := c.AgentOracleConstructionContext(literal)
		if undiscriminated == nil {
			row["excluded"] = "context"
			continue
		}
		// The context as discrimination leaves it: the instantiated contextual
		// type, or the discriminated apparent type when it removed a member.
		context := instantiated
		if discriminated != undiscriminated {
			context = discriminated
		}
		row["context"], row["contextError"] = c.TypeToString(context), c.AgentOracleIsErrorType(undiscriminated)
		constituents := func(t *checker.Type) []*checker.Type {
			if t.Flags()&checker.TypeFlagsUnion != 0 {
				return t.Types()
			}
			return []*checker.Type{t}
		}
		// Independent of the identity rule below: a constituent of the
		// instantiated context, or of its base constraint, is the declared
		// type or one of its alternatives, or carries the declaration's alias
		// symbol.
		isTarget := func(t *checker.Type) bool {
			return t == declared || slices.Contains(alternatives, t) || t.Alias() != nil && t.Alias().Symbol() == symbol
		}
		contextIsTarget := false
		for _, constituent := range constituents(instantiated) {
			if isTarget(constituent) {
				contextIsTarget = true
			}
			if constraint := c.AgentOracleBaseConstraint(constituent); constraint != nil && slices.ContainsFunc(constituents(constraint), isTarget) {
				contextIsTarget = true
			}
		}
		row["contextIsTarget"] = contextIsTarget
		remaining := constituents(discriminated)
		match := func(t *checker.Type) int {
			for candidate, alternative := range alternatives {
				if c.AgentOracleInstanceOf(t, alternative, symbol, generic) {
					return candidate
				}
			}
			return -1
		}
		// Identity is decided on each constituent of the instantiated context,
		// whose apparent form may be rebuilt without its alias (an
		// intersection's getTypeWithThisArgument). A constituent that is not
		// an instance is tried through its base constraint when it is
		// instantiable (rebuilt likewise by its apparent form), and through
		// its apparent forms otherwise. Discrimination keeps or removes the
		// apparent forms; an alternative reached twice is one construction.
		type candidate struct {
			instance *checker.Type
			index    int
			forms    []*checker.Type
		}
		candidates := []candidate{}
		add := func(instance *checker.Type, index int, forms []*checker.Type) {
			if !slices.ContainsFunc(candidates, func(found candidate) bool { return found.instance == instance && found.index == index }) {
				candidates = append(candidates, candidate{instance, index, forms})
			}
		}
		for _, constituent := range constituents(instantiated) {
			apparent := c.AgentOracleContextualApparent(constituent)
			apparentForms := constituents(apparent)
			if index := match(constituent); index >= 0 {
				add(constituent, index, apparentForms)
				continue
			}
			if apparent == constituent {
				continue
			}
			identities := apparentForms
			if constituent.Flags()&checker.TypeFlagsInstantiable != 0 {
				identities = nil
				if constraint := c.AgentOracleBaseConstraint(constituent); constraint != nil {
					identities = constituents(constraint)
				}
			}
			for _, form := range identities {
				if index := match(form); index >= 0 {
					if slices.Contains(apparentForms, form) {
						add(form, index, []*checker.Type{form})
					} else {
						add(form, index, apparentForms)
					}
				}
			}
		}
		constructed := []map[string]any{}
		discriminatedAway := 0
		var literalType *checker.Type
		for _, found := range candidates {
			if !slices.ContainsFunc(found.forms, func(form *checker.Type) bool { return slices.Contains(remaining, form) }) {
				discriminatedAway++
				continue
			}
			if literalType == nil {
				literalType = c.GetTypeAtLocation(literal)
				row["literalError"] = c.AgentOracleIsErrorType(literalType)
			}
			properties, optional := c.AgentOracleTargetProperties(found.instance)
			lacks := []map[string]any{}
			for position, property := range properties {
				if presence := c.AgentOraclePresence(literalType, property.Name); presence != "present" {
					lack := map[string]any{"optional": optional[position], "presence": presence}
					for name, value := range publicKey(property.Name) {
						lack[name] = value
					}
					lacks = append(lacks, lack)
				}
			}
			constructed = append(constructed, map[string]any{"alternative": found.index, "summary": c.TypeToString(found.instance), "lacks": lacks})
		}
		if len(constructed) != 0 || discriminatedAway != 0 {
			row["constructed"], row["discriminatedAway"] = constructed, discriminatedAway
		}
	}
	result["literals"] = rows
	return result
}

// publicKey spells a property's name as the product's TypeKey does: an
// ECMAScript private name and a unique symbol's name are internal encodings
// (prefix, then `#id@#name` or `@name@id`), which give the public name and a
// kind; any other name is a string key.
func publicKey(name string) map[string]any {
	if internal, ok := strings.CutPrefix(name, ast.InternalSymbolNamePrefix); ok {
		if rest, ok := strings.CutPrefix(internal, "#"); ok {
			if _, private, ok := strings.Cut(rest, "@"); ok {
				return map[string]any{"key": private, "kind": "private"}
			}
		}
		if rest, ok := strings.CutPrefix(internal, "@"); ok {
			if at := strings.LastIndex(rest, "@"); at >= 0 {
				return map[string]any{"key": rest[:at], "kind": "symbol"}
			}
		}
	}
	return map[string]any{"key": name}
}

// inDefaultUniverse: a literal is examined when one of its named members
// (not a spread, and with no dynamic name, ast.IsDynamicName) has a key
// (AgentOracleMemberKey) some alternative declares, or when it has no named
// member at all.
func inDefaultUniverse(c *checker.Checker, literal *ast.Node, keys map[string]bool) bool {
	named := false
	for _, member := range literal.AsObjectLiteralExpression().Properties.Nodes {
		if member.Kind == ast.KindSpreadAssignment {
			continue
		}
		if ast.IsDynamicName(member.Name()) {
			continue
		}
		named = true
		if member.Symbol() == nil || keys[c.AgentOracleMemberKey(member)] {
			return true
		}
	}
	return !named
}

// objectLike: an object type, or an intersection made only of object types.
func objectLike(t *checker.Type) bool {
	if t.Flags()&checker.TypeFlagsObject != 0 {
		return true
	}
	if t.Flags()&checker.TypeFlagsIntersection == 0 {
		return false
	}
	for _, constituent := range t.Types() {
		if !objectLike(constituent) {
			return false
		}
	}
	return true
}

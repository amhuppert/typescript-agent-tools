// Test-only overlay of the pinned TypeScript 7.0.2 tspath package.
package tspath

import (
	"encoding/json"
	"os"
	"testing"
)

func TestAgentRelativePaths(t *testing.T) {
	type Row struct {
		FromPath    string `json:"fromPath"`
		ToPath      string `json:"toPath"`
		Cwd         string `json:"cwd"`
		Insensitive bool   `json:"insensitive"`
		Expected    string `json:"expected"`
	}
	data, err := os.ReadFile(os.Getenv("AGENT_PATH_CASES"))
	if err != nil {
		t.Fatal(err)
	}
	var rows []Row
	if err := json.Unmarshal(data, &rows); err != nil {
		t.Fatal(err)
	}
	for i := range rows {
		row := &rows[i]
		row.Expected = GetRelativePathToDirectoryOrUrl(row.FromPath, row.ToPath, false, ComparePathsOptions{
			CurrentDirectory:          row.Cwd,
			UseCaseSensitiveFileNames: !row.Insensitive,
		})
	}
	result, err := json.MarshalIndent(rows, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(os.Getenv("AGENT_PATH_OUTPUT"), append(result, '\n'), 0600); err != nil {
		t.Fatal(err)
	}
}

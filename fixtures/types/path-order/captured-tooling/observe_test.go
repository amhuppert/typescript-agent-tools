package stringutil

import (
    "encoding/json"
    "os"
    "slices"
    "testing"
)

func TestAgentPathOrder(t *testing.T) {
    names := []string{"a.ts", "\U00010000.ts", "\ue000.ts"}
    comparison := CompareStringsCaseSensitive(names[1], names[2])
    slices.SortFunc(names, CompareStringsCaseSensitive)
    bytes, err := json.Marshal(map[string]any{"astralVersusBMP": comparison, "exactOrder": names})
    if err != nil { t.Fatal(err) }
    if err := os.WriteFile(os.Getenv("AGENT_PATH_ORDER_OUTPUT"), bytes, 0600); err != nil { t.Fatal(err) }
}

package session

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strconv"
)

var namePattern = regexp.MustCompile(`^[a-zA-Z0-9_-]{1,64}$`)

// ValidateName checks that a session name is non-empty and filesystem-safe.
func ValidateName(name string) error {
	if name == "" {
		return fmt.Errorf("session name required")
	}
	if !namePattern.MatchString(name) {
		return fmt.Errorf("invalid session name %q (use alphanumeric, hyphens, underscores)", name)
	}
	return nil
}

// Dir returns the path to a session's root directory.
func Dir(projectRoot, name string) string {
	return filepath.Join(projectRoot, ".snap", "sessions", name)
}

// TasksDir returns the path to a session's tasks directory.
func TasksDir(projectRoot, name string) string {
	return filepath.Join(Dir(projectRoot, name), "tasks")
}

// Exists checks whether a session directory exists.
func Exists(projectRoot, name string) bool {
	info, err := os.Stat(Dir(projectRoot, name))
	return err == nil && info.IsDir()
}

// Create validates the session name and creates the session directory structure.
func Create(projectRoot, name string) error {
	if err := ValidateName(name); err != nil {
		return err
	}
	if Exists(projectRoot, name) {
		return fmt.Errorf("session '%s' already exists", name)
	}
	return os.MkdirAll(TasksDir(projectRoot, name), 0o755)
}

// EnsureDefault creates a "default" session if it does not already exist.
// It is idempotent — calling it when the session already exists is a no-op.
func EnsureDefault(projectRoot string) error {
	if Exists(projectRoot, "default") {
		return nil
	}
	return Create(projectRoot, "default")
}

// Info describes a session's name, task counts, and derived status.
type Info struct {
	Name           string
	TaskCount      int
	CompletedCount int
	Status         string
}

// sessionsDir returns the path to the sessions root directory.
func sessionsDir(projectRoot string) string {
	return filepath.Join(projectRoot, ".snap", "sessions")
}

// taskFileRegex matches TASK<n>.md filenames (uppercase only).
var taskFileRegex = regexp.MustCompile(`^TASK\d+\.md$`)

// statusTaskRegex matches TASK<n>.md filenames with capture group for the numeric part.
var statusTaskRegex = regexp.MustCompile(`^TASK(\d+)\.md$`)

// List scans .snap/sessions/ and returns info for each session, sorted by name.
func List(projectRoot string) ([]Info, error) {
	dir := sessionsDir(projectRoot)
	entries, err := os.ReadDir(dir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("read sessions directory: %w", err)
	}

	var sessions []Info
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		info := Info{Name: entry.Name()}
		sessionPath := filepath.Join(dir, entry.Name())

		// Count task files.
		tasksPath := filepath.Join(sessionPath, "tasks")
		if taskEntries, err := os.ReadDir(tasksPath); err == nil {
			for _, te := range taskEntries {
				if !te.IsDir() && taskFileRegex.MatchString(te.Name()) {
					info.TaskCount++
				}
			}
		}

		// Read state.json for completed count and step info.
		statePath := filepath.Join(sessionPath, "state.json")
		stateData, stateErr := os.ReadFile(statePath)

		var st *sessionState
		stateCorrupt := false
		if stateErr == nil {
			var parsed sessionState
			if json.Unmarshal(stateData, &parsed) == nil {
				st = &parsed
				info.CompletedCount = len(parsed.CompletedTaskIDs)
			} else {
				stateCorrupt = true
			}
		}

		// Check for .plan-started marker.
		planMarkerPath := filepath.Join(sessionPath, ".plan-started")
		_, planErr := os.Stat(planMarkerPath)
		hasPlanning := planErr == nil

		// Derive status.
		info.Status = deriveStatus(info.TaskCount, st, stateCorrupt, hasPlanning)

		sessions = append(sessions, info)
	}

	sort.Slice(sessions, func(i, j int) bool {
		return sessions[i].Name < sessions[j].Name
	})

	return sessions, nil
}

// sessionState is a minimal struct to read state.json fields needed for status derivation.
// Note: Keep fields in sync with internal/state/types.go State struct.
type sessionState struct {
	CurrentTaskID    string   `json:"current_task_id"`
	CurrentStep      int      `json:"current_step"`
	TotalSteps       int      `json:"total_steps"`
	CompletedTaskIDs []string `json:"completed_task_ids"`
}

func deriveStatus(taskCount int, st *sessionState, stateCorrupt, hasPlanning bool) string {
	if stateCorrupt {
		return "unknown"
	}

	completedCount := 0
	if st != nil {
		completedCount = len(st.CompletedTaskIDs)
	}

	if hasPlanning && completedCount == 0 {
		return "planning"
	}

	if taskCount == 0 && !hasPlanning {
		return "no tasks"
	}

	if st != nil {
		if taskCount > 0 && completedCount >= taskCount {
			return "complete"
		}
		if st.CurrentTaskID != "" && st.CurrentStep > 0 {
			return fmt.Sprintf("paused at step %d", st.CurrentStep)
		}
		return "idle"
	}

	return "idle"
}

// Resolve validates that a named session exists and returns its directory path.
func Resolve(projectRoot, name string) (string, error) {
	if err := ValidateName(name); err != nil {
		return "", err
	}
	if !Exists(projectRoot, name) {
		return "", fmt.Errorf("session '%s' not found — create it with: snap new %s", name, name)
	}
	return Dir(projectRoot, name), nil
}

// HasPlanHistory checks whether a session has a .plan-started marker file.
func HasPlanHistory(projectRoot, name string) bool {
	markerPath := filepath.Join(Dir(projectRoot, name), ".plan-started")
	_, err := os.Stat(markerPath)
	return err == nil
}

// MarkPlanStarted creates the .plan-started marker file in the session directory.
func MarkPlanStarted(projectRoot, name string) error {
	markerPath := filepath.Join(Dir(projectRoot, name), ".plan-started")
	return os.WriteFile(markerPath, []byte(""), 0o600)
}

// artifactNames are exact filenames considered planning artifacts.
var artifactNames = map[string]bool{
	"BRIEF.md":      true,
	"PRD.md":        true,
	"TECHNOLOGY.md": true,
	"DESIGN.md":     true,
	"TASKS.md":      true,
}

// HasArtifacts reports whether a session's tasks directory contains any
// planning artifacts (TASK*.md, PRD.md, TECHNOLOGY.md, DESIGN.md).
// Returns false on read error or empty directory.
func HasArtifacts(projectRoot, name string) bool {
	td := TasksDir(projectRoot, name)
	entries, err := os.ReadDir(td)
	if err != nil {
		return false
	}
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if taskFileRegex.MatchString(entry.Name()) || artifactNames[entry.Name()] {
			return true
		}
	}
	return false
}

// CleanSession removes all files from the session's tasks directory,
// state.json, and .plan-started marker. Leaves the session directory intact.
// Missing files are ignored.
func CleanSession(projectRoot, name string) error {
	td := TasksDir(projectRoot, name)
	entries, err := os.ReadDir(td)
	if err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("read tasks directory: %w", err)
	}
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if err := os.Remove(filepath.Join(td, entry.Name())); err != nil {
			return fmt.Errorf("remove %s: %w", entry.Name(), err)
		}
	}

	sd := Dir(projectRoot, name)
	for _, f := range []string{"state.json", ".plan-started", ".plan-tier"} {
		if err := os.Remove(filepath.Join(sd, f)); err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("remove %s: %w", f, err)
		}
	}

	return nil
}

// TaskStatus describes one task file's completion state.
type TaskStatus struct {
	ID        string
	Completed bool
}

// StatusInfo holds detailed information about a session.
type StatusInfo struct {
	Name       string
	TasksDir   string
	Tasks      []TaskStatus
	ActiveTask string
	ActiveStep int
	TotalSteps int
}

// Status returns detailed status for a named session.
func Status(projectRoot, name string) (*StatusInfo, error) {
	if _, err := Resolve(projectRoot, name); err != nil {
		return nil, err
	}

	td := TasksDir(projectRoot, name)
	sessionDir := Dir(projectRoot, name)

	// Scan task files.
	entries, err := os.ReadDir(td)
	if err != nil && !os.IsNotExist(err) {
		return nil, fmt.Errorf("read tasks directory: %w", err)
	}

	type taskEntry struct {
		id     string
		number int
	}
	var tasks []taskEntry
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		matches := statusTaskRegex.FindStringSubmatch(entry.Name())
		if matches == nil {
			continue
		}
		num, err := strconv.Atoi(matches[1])
		if err != nil {
			continue
		}
		id := fmt.Sprintf("TASK%s", matches[1])
		tasks = append(tasks, taskEntry{id: id, number: num})
	}
	sort.Slice(tasks, func(i, j int) bool {
		return tasks[i].number < tasks[j].number
	})

	// Read state.json for completion info.
	statePath := filepath.Join(sessionDir, "state.json")
	var st *sessionState
	if stateData, stateErr := os.ReadFile(statePath); stateErr == nil {
		var parsed sessionState
		if json.Unmarshal(stateData, &parsed) == nil {
			st = &parsed
		}
	}

	completedSet := make(map[string]bool)
	if st != nil {
		for _, id := range st.CompletedTaskIDs {
			completedSet[id] = true
		}
	}

	result := &StatusInfo{
		Name:     name,
		TasksDir: td,
	}

	for _, t := range tasks {
		result.Tasks = append(result.Tasks, TaskStatus{
			ID:        t.id,
			Completed: completedSet[t.id],
		})
	}

	if st != nil && st.CurrentTaskID != "" {
		result.ActiveTask = st.CurrentTaskID
		result.ActiveStep = st.CurrentStep
		result.TotalSteps = st.TotalSteps
	}

	return result, nil
}

// Delete removes a session directory and all its contents.
func Delete(projectRoot, name string) error {
	if err := ValidateName(name); err != nil {
		return err
	}
	if !Exists(projectRoot, name) {
		return fmt.Errorf("session '%s' not found", name)
	}
	return os.RemoveAll(Dir(projectRoot, name))
}

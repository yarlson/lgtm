package plan

import (
	"context"
	"fmt"
	"io"
	"sync/atomic"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/yarlson/snap/internal/model"
)

// slowExecutor introduces a configurable delay before returning.
type slowExecutor struct {
	delay time.Duration
	err   error
}

func (e *slowExecutor) Run(ctx context.Context, w io.Writer, _ model.Type, _ ...string) error {
	select {
	case <-time.After(e.delay):
	case <-ctx.Done():
		return ctx.Err()
	}
	if e.err != nil {
		return e.err
	}
	fmt.Fprintln(w, "done")
	return nil
}

// perTaskExecutor returns a different error per task name.
type perTaskExecutor struct {
	errors map[string]error
}

func (e *perTaskExecutor) Run(_ context.Context, w io.Writer, _ model.Type, args ...string) error {
	// The task name is embedded as the last arg by the test setup.
	name := args[len(args)-1]
	if err, ok := e.errors[name]; ok && err != nil {
		return err
	}
	fmt.Fprintln(w, "done")
	return nil
}

func TestRunParallel_BothSucceed(t *testing.T) {
	exec := &slowExecutor{delay: 10 * time.Millisecond}
	tasks := []parallelTask{
		{name: "Technology plan", modelType: model.Thinking, args: []string{"tech-prompt"}},
		{name: "Design spec", modelType: model.Thinking, args: []string{"design-prompt"}},
	}

	results := runParallel(context.Background(), exec, tasks, 0)

	require.Len(t, results, 2)
	for _, r := range results {
		assert.NoError(t, r.err, "task %q should succeed", r.name)
		assert.Greater(t, r.elapsed, time.Duration(0), "task %q should have positive elapsed", r.name)
	}
}

func TestRunParallel_OneFails(t *testing.T) {
	exec := &perTaskExecutor{
		errors: map[string]error{
			"tech-prompt":   nil,
			"design-prompt": fmt.Errorf("design generation failed"),
		},
	}
	tasks := []parallelTask{
		{name: "Technology plan", modelType: model.Thinking, args: []string{"tech-prompt"}},
		{name: "Design spec", modelType: model.Thinking, args: []string{"design-prompt"}},
	}

	results := runParallel(context.Background(), exec, tasks, 0)

	require.Len(t, results, 2)

	// Find which succeeded and which failed.
	var successCount, failCount int
	for _, r := range results {
		if r.err != nil {
			failCount++
			assert.Contains(t, r.err.Error(), "design generation failed")
		} else {
			successCount++
		}
	}
	assert.Equal(t, 1, successCount)
	assert.Equal(t, 1, failCount)
}

func TestRunParallel_ContextCancel(t *testing.T) {
	exec := &slowExecutor{delay: 5 * time.Second}
	tasks := []parallelTask{
		{name: "Technology plan", modelType: model.Thinking, args: []string{"tech-prompt"}},
		{name: "Design spec", modelType: model.Thinking, args: []string{"design-prompt"}},
	}

	ctx, cancel := context.WithCancel(context.Background())
	// Cancel after a short delay.
	go func() {
		time.Sleep(50 * time.Millisecond)
		cancel()
	}()

	results := runParallel(ctx, exec, tasks, 0)

	require.Len(t, results, 2)
	for _, r := range results {
		assert.Error(t, r.err, "task %q should be cancelled", r.name)
	}
}

func TestRunParallel_AllFail(t *testing.T) {
	exec := &perTaskExecutor{
		errors: map[string]error{
			"tech-prompt":   fmt.Errorf("tech failed"),
			"design-prompt": fmt.Errorf("design failed"),
		},
	}
	tasks := []parallelTask{
		{name: "Technology plan", modelType: model.Thinking, args: []string{"tech-prompt"}},
		{name: "Design spec", modelType: model.Thinking, args: []string{"design-prompt"}},
	}

	results := runParallel(context.Background(), exec, tasks, 0)

	require.Len(t, results, 2)
	for _, r := range results {
		assert.Error(t, r.err, "task %q should fail", r.name)
	}
}

func TestRunParallel_OutputIsolation(t *testing.T) {
	// Verify each goroutine writes to its own buffer (no interleaving).
	exec := &recordingExecutor{}
	tasks := []parallelTask{
		{name: "Technology plan", modelType: model.Thinking, args: []string{"tech-prompt"}},
		{name: "Design spec", modelType: model.Thinking, args: []string{"design-prompt"}},
	}

	results := runParallel(context.Background(), exec, tasks, 0)

	require.Len(t, results, 2)
	for _, r := range results {
		assert.NoError(t, r.err)
		// mockExecutor writes "LLM response\n" to the writer.
		assert.Equal(t, "LLM response\n", r.output.String(),
			"task %q should have isolated output", r.name)
	}
}

// concurrencyTrackingExecutor tracks the maximum number of in-flight calls.
type concurrencyTrackingExecutor struct {
	delay    time.Duration
	inflight atomic.Int32
	maxSeen  atomic.Int32
}

func (e *concurrencyTrackingExecutor) Run(ctx context.Context, w io.Writer, _ model.Type, _ ...string) error {
	cur := e.inflight.Add(1)
	defer e.inflight.Add(-1)

	// Update max seen atomically.
	for {
		old := e.maxSeen.Load()
		if cur <= old || e.maxSeen.CompareAndSwap(old, cur) {
			break
		}
	}

	select {
	case <-time.After(e.delay):
	case <-ctx.Done():
		return ctx.Err()
	}
	fmt.Fprintln(w, "done")
	return nil
}

func TestRunParallel_WithLimit(t *testing.T) {
	exec := &concurrencyTrackingExecutor{delay: 50 * time.Millisecond}

	// 10 tasks with limit 5 — no more than 5 should run concurrently.
	tasks := make([]parallelTask, 0, 10)
	for i := range 10 {
		tasks = append(tasks, parallelTask{
			name:      fmt.Sprintf("task-%d", i),
			modelType: model.Thinking,
			args:      []string{fmt.Sprintf("prompt-%d", i)},
		})
	}

	results := runParallel(context.Background(), exec, tasks, 5)

	require.Len(t, results, 10)
	for _, r := range results {
		assert.NoError(t, r.err, "task %q should succeed", r.name)
	}

	assert.LessOrEqual(t, exec.maxSeen.Load(), int32(5),
		"no more than 5 tasks should run concurrently")
	assert.Greater(t, exec.maxSeen.Load(), int32(1),
		"at least 2 tasks should run concurrently")
}

func TestRunParallel_BatchPartialFailure(t *testing.T) {
	exec := &perTaskExecutor{
		errors: map[string]error{
			"prompt-1": fmt.Errorf("task 1 failed"),
			"prompt-3": fmt.Errorf("task 3 failed"),
		},
	}

	tasks := make([]parallelTask, 0, 5)
	for i := range 5 {
		tasks = append(tasks, parallelTask{
			name:      fmt.Sprintf("task-%d", i),
			modelType: model.Thinking,
			args:      []string{fmt.Sprintf("prompt-%d", i)},
		})
	}

	results := runParallel(context.Background(), exec, tasks, 5)

	require.Len(t, results, 5)

	var successCount, failCount int
	for _, r := range results {
		if r.err != nil {
			failCount++
		} else {
			successCount++
		}
	}
	assert.Equal(t, 3, successCount)
	assert.Equal(t, 2, failCount)
}

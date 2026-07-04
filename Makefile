.PHONY: all build check clippy fmt test release eval-check clean

all: check

build:
	cargo build --all-targets --all-features

check: fmt clippy test build

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt --all --check

test:
	cargo test --all-features

release:
	cargo build --release --all-targets --all-features

eval-check:
	cargo build
	python3 -B -m compileall -q evals
	python3 -m unittest discover -s evals/tests
	python3 evals/run-gate-negative/run_eval.py --include-pass-control
	python3 evals/plan-create/run_eval.py --score-only evals/plan-create/controls/weak-kargo-keyword-stuffed-14-phase.md --expect-fail
	python3 evals/plan-create/run_eval.py --score-only evals/plan-create/controls/good-kargo-job-system.md
	python3 evals/shape-quality/run_eval.py --score-only evals/shape-quality/controls/weak-shape-finalizes-too-early.md --expect-fail
	find evals -type d -name __pycache__ -prune -exec rm -rf {} +

clean:
	cargo clean

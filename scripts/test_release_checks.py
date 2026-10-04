"""Gearu's release check: it refuses before any gate while publish = false or the
version differs, then runs the candidate gates in order and stops at the first that
fails. Packaging runs in the exact stage, on the release commit: gearu checks its
candidate before committing it, and `cargo package` refuses uncommitted files."""
from pathlib import Path
import subprocess
import tempfile
import unittest

import release_checks


def repository(directory: str, version: str, publish: str | None) -> Path:
    root = Path(directory)
    text = f'[package]\nname = "gwz-sspi"\nversion = "{version}"\n'
    if publish is not None:
        text += f"publish = {publish}\n"
    (root / "Cargo.toml").write_text(text)
    return root


class Recorder:
    def __init__(self, failing: int | None = None):
        self.commands: list[list[str]] = []
        self.failing = failing

    def __call__(self, command, *, cwd, check):
        self.commands.append(command)
        code = 3 if len(self.commands) - 1 == self.failing else 0
        return subprocess.CompletedProcess(command, code)


class ReleaseCheckTests(unittest.TestCase):
    def test_a_version_other_than_the_manifests_is_refused_in_both_stages(self):
        for exact in (False, True):
            with tempfile.TemporaryDirectory() as directory:
                run = Recorder()
                root = repository(directory, "0.1.0", None)
                self.assertEqual(release_checks.check(root, "0.1.1", run, exact=exact), 1)
                self.assertEqual(run.commands, [])

    def test_publish_false_refuses_in_both_stages_before_any_gate(self):
        for exact in (False, True):
            with tempfile.TemporaryDirectory() as directory:
                run = Recorder()
                root = repository(directory, "0.1.0", "false")
                self.assertEqual(release_checks.check(root, "0.1.0", run, exact=exact), 1)
                self.assertEqual(run.commands, [])

    def test_the_candidate_stage_runs_the_gates_in_order_without_packaging(self):
        with tempfile.TemporaryDirectory() as directory:
            run = Recorder()
            self.assertEqual(release_checks.check(repository(directory, "0.1.0", None), "0.1.0", run), 0)
        self.assertEqual(run.commands, [
            ["cargo", "fmt", "--all", "--", "--check"],
            ["cargo", "clippy", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings"],
            ["cargo", "test", "--all-features", "--locked"],
        ])

    def test_the_exact_stage_packages_the_release_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            run = Recorder()
            root = repository(directory, "0.1.0", None)
            self.assertEqual(release_checks.check(root, "0.1.0", run, exact=True), 0)
        self.assertEqual(run.commands, [["cargo", "package", "--locked"]])

    def test_the_first_failing_gate_ends_the_check_with_its_status(self):
        with tempfile.TemporaryDirectory() as directory:
            run = Recorder(failing=1)
            self.assertEqual(release_checks.check(repository(directory, "0.1.0", None), "0.1.0", run), 3)
        self.assertEqual(len(run.commands), 2)


if __name__ == "__main__":
    unittest.main()

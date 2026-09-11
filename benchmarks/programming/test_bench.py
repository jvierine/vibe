"""Harness invariants. Synthetic telemetry here is never experimental evidence."""
import argparse
import contextlib
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import bench


class HarnessTests(unittest.TestCase):
    def setUp(self):
        self.folder=tempfile.TemporaryDirectory(prefix="vibe-token-bench-test-")
        self.root=Path(self.folder.name)

    def tearDown(self): self.folder.cleanup()

    def prepare(self,tokens=1000,seconds=60):
        return bench.prepare(argparse.Namespace(root=self.root,language="python",task="trapezoid",
            seed=4,model="synthetic-unit-test",cohort="agent",seconds=seconds,tokens=tokens))

    def usage(self,trial,inputs=20,outputs=10,complete=False):
        with patch("sys.stdin",io.StringIO(bench.json.dumps(dict(input_tokens=inputs,output_tokens=outputs,tool_calls=2,source="synthetic unit test",complete=complete)))):
            bench.usage(trial)

    def test_failed_repair_and_final_artifact_validation(self):
        trial=self.prepare(); bench.start(trial)
        self.assertFalse(bench.evaluate(trial)["passed"])
        bench.install(trial/"workspace","python","trapezoid",True)
        self.assertTrue(bench.evaluate(trial)["passed"])
        # A passing checkpoint cannot certify a later broken edit.
        (trial/"workspace/solution.py").write_text("def solve(data): return float('nan')\n")
        self.assertFalse(bench.finish(trial)["passed"])
        with bench.state(trial) as data:
            self.assertFalse(data.attrs["comparison_eligible"])
            self.assertEqual(data.attrs["tokens"],-1)
        with self.assertRaises(ValueError): bench.evaluate(trial)

    def test_cumulative_usage_idempotence_and_budget_failure(self):
        trial=self.prepare(tokens=35); bench.start(trial,True)
        self.usage(trial); self.usage(trial)
        self.usage(trial,30,10,complete=True)
        with self.assertRaises(ValueError): self.usage(trial,29,10)
        bench.install(trial/"workspace","python","trapezoid",True)
        self.assertTrue(bench.finish(trial)["passed"])
        with bench.state(trial) as data:
            self.assertEqual(len(data["usage_snapshots"]),2)
            self.assertEqual(data.attrs["tokens"],40)
            self.assertTrue(data.attrs["comparison_eligible"])
            self.assertFalse(data.attrs["measured_success"])

    def test_managed_timeout_is_a_failure_not_a_dropped_trial(self):
        trial=self.prepare(seconds=.2)
        bench.install(trial/"workspace","python","trapezoid",True)
        bench.run(trial,[sys.executable,"-c","import time; time.sleep(5)"])
        with bench.state(trial) as data:
            self.assertTrue(data.attrs["driver_timed_out"])
            self.assertFalse(data.attrs["measured_success"])
            self.assertFalse(data.attrs["comparison_eligible"])
        output=io.StringIO()
        with contextlib.redirect_stdout(output): bench.summary(self.root)
        self.assertIn("Missing telemetry",output.getvalue())
        self.assertIn("| python | 1 | 1 | 1 | 0 | unknown |",output.getvalue())

    def test_partial_tokens_do_not_qualify_as_complete_usage(self):
        trial=self.prepare(); bench.start(trial,True); self.usage(trial)
        bench.install(trial/"workspace","python","trapezoid",True)
        self.assertTrue(bench.finish(trial)["passed"])
        with bench.state(trial) as data:
            self.assertEqual(data.attrs["observed_tokens"],30)
            self.assertEqual(data.attrs["tokens"],-1)
            self.assertFalse(data.attrs["within_budget"])
            self.assertFalse(data.attrs["comparison_eligible"])

    def test_python_mutation_is_rejected(self):
        trial=self.prepare(); bench.start(trial)
        (trial/"workspace/solution.py").write_text("def solve(data):\n    data[0]=999.0\n    return 0.0\n")
        self.assertFalse(bench.evaluate(trial)["passed"])


if __name__=="__main__": unittest.main()

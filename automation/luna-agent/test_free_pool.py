#!/usr/bin/env python3
from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import runner

class FreePoolTests(unittest.TestCase):
    def test_failure_classification(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / "log"
            log.write_text("HTTP 429 free-models-per-day", encoding="utf-8")
            self.assertEqual(
                runner.classify_free_failure(log),
                ("provider", "global-rate-limit"),
            )
            log.write_text("error: model was not found", encoding="utf-8")
            self.assertEqual(
                runner.classify_free_failure(log),
                ("model", "model-limit"),
            )

    def test_max_tokens_text_is_not_context_failure(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / "log"
            log.write_text("failed after setting max_tokens=65536", encoding="utf-8")
            self.assertIsNone(runner.classify_free_failure(log))

    def test_global_provider_scope_is_skipped(self) -> None:
        openrouter = next(
            i for i, item in enumerate(runner.FREE_POOL)
            if item.scope == "openrouter"
        )
        with mock.patch.object(runner, "pool_entry_available", return_value=True):
            next_index = runner.next_free_pool_index(
                openrouter,
                skip_scope="openrouter",
                state={},
            )
        self.assertIsNotNone(next_index)
        self.assertNotEqual(runner.free_pool_entry(next_index).scope, "openrouter")

    def test_fallback_preserves_turn_and_changes_provider(self) -> None:
        current = next(
            i for i, item in enumerate(runner.FREE_POOL)
            if item.scope == "openrouter"
        )
        info = {
            "free_pool_index": current,
            "turns": 1,
            "status": "running",
        }
        state = {"tasks": {}, "runner_status": "working"}
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / "log"
            log.write_text("status code 429", encoding="utf-8")
            with (
                mock.patch.object(runner, "git_status", return_value=[]),
                mock.patch.object(runner, "pool_entry_available", return_value=True),
                mock.patch.object(runner, "save_state"),
            ):
                changed = runner.advance_free_pool(state, info, log)
        self.assertTrue(changed)
        self.assertEqual(info["turns"], 0)
        self.assertEqual(info["phase"], "model-fallback")
        self.assertNotEqual(info["free_pool_index"], current)

if __name__ == "__main__":
    unittest.main()

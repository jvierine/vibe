"""Regression tests for standalone native build caching (base conda Python)."""
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import tempfile
import unittest
from native_build import compile_shared


class NativeCacheTests(unittest.TestCase):
    def test_reuse_source_change_sanitizer_and_corruption(self):
        with tempfile.TemporaryDirectory(prefix="vibe-cache-test-") as folder:
            source=b"double value(void) { return 1.0; }\n"
            first=compile_shared(source,folder)
            self.assertFalse(first.cache_hit)
            self.assertTrue(compile_shared(source,folder).cache_hit)
            second=compile_shared(source.replace(b"1.0",b"2.0"),folder)
            self.assertNotEqual(first.key,second.key)
            sanitized=compile_shared(source,folder,sanitize=True)
            self.assertNotEqual(first.key,sanitized.key)
            first.path.write_bytes(b"corrupt test artifact")
            self.assertFalse(compile_shared(source,folder).cache_hit)
            self.assertTrue(compile_shared(source,folder).cache_hit)

    def test_concurrent_same_key_builds_only_once(self):
        with tempfile.TemporaryDirectory(prefix="vibe-cache-parallel-") as folder:
            with ThreadPoolExecutor(max_workers=4) as pool:
                artifacts=list(pool.map(lambda _:compile_shared(b"double answer(void) { return 42.0; }",Path(folder)),range(4)))
            self.assertEqual(sum(not a.cache_hit for a in artifacts),1)
            self.assertEqual(len({a.path for a in artifacts}),1)


if __name__=="__main__": unittest.main()

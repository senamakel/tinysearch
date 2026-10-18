"""Exercise routing and input guards in the module release workflow."""

import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/release.yml"
CI_WORKFLOW = ROOT / ".github/workflows/ci.yml"
REVIEWED_RELEASE_SHA = "e045fc567c583a1b83dc9c1edef896179853fa69"
ORDINARY_RELEASE_SHA = "4adc1080fb495098e7af1aade82ed7e6e3c69bb7"
TINYBUS_SDK_SHA = "cba2de0e573f99fe4c7e45ba14e2d6b5a2a7757f"


def resolve_script(workflow):
    match = re.search(
        r"(?ms)^      - id: tag\n.*?^        run: \|\n((?:^          .*\n|^\n)+)",
        workflow,
    )
    if not match:
        raise AssertionError("release resolve script was not found")
    return "\n".join(
        line[10:] if line.startswith("          ") else ""
        for line in match[1].splitlines()
    )


class ReleaseWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workflow = WORKFLOW.read_text()
        cls.ci_workflow = CI_WORKFLOW.read_text()
        cls.script = resolve_script(cls.workflow)

    def run_resolve(self, mode="main", reviewed_pr="", ref="refs/heads/main"):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            env = dict(
                os.environ,
                TAG="v0.5.2",
                RELEASE_MODE=mode,
                REVIEWED_PR=reviewed_pr,
                GITHUB_REF=ref,
                GITHUB_OUTPUT=str(output),
            )
            result = subprocess.run(
                ["bash", "-c", self.script],
                env=env,
                text=True,
                capture_output=True,
            )
            return result, output.read_text() if output.exists() else ""

    def test_ordinary_tag_release_route_and_module_ci_pin_are_preserved(self):
        self.assertIn("push:\n    tags: ['v*']", self.workflow)
        self.assertIn("github.event_name == 'push' && github.ref_name || inputs.tag", self.workflow)
        self.assertIn("release_mode=main", self.run_resolve("main")[1])
        self.assertIn(
            f"uses: tinyhumansai/.github/.github/workflows/module-release.yml@{ORDINARY_RELEASE_SHA}",
            self.workflow,
        )
        self.assertIn(
            f"uses: tinyhumansai/.github/.github/workflows/module-ci.yml@{ORDINARY_RELEASE_SHA}",
            self.ci_workflow,
        )
        self.assertIn("coverage-script: scripts/check-coverage.sh", self.ci_workflow)
        self.assertIn("module-e2e-script: scripts/test-e2e.sh", self.ci_workflow)
        self.assertIn("feature-test-script: scripts/test-features.sh", self.ci_workflow)

    def test_published_candidate_and_sdk_constraints_match_released_compatibility(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertEqual(manifest["workspace"]["package"]["version"], "0.5.2")
        self.assertEqual(
            manifest["workspace"]["dependencies"]["tinybus"]["version"],
            ">=0.3.2, <0.4",
        )
        self.assertEqual(
            manifest["workspace"]["dependencies"]["tinybus-module"]["version"],
            ">=0.3.2, <0.4",
        )
        bus_head = subprocess.run(
            ["git", "-C", str(ROOT / "vendor/tinybus"), "rev-parse", "HEAD"],
            check=True,
            text=True,
            capture_output=True,
        ).stdout.strip()
        self.assertEqual(bus_head, TINYBUS_SDK_SHA)

    def test_reviewed_release_requires_positive_pr_on_main_and_uses_fixed_profile(self):
        result, output = self.run_resolve("reviewed-epic", "83")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("tag=v0.5.2", output)
        self.assertIn("version=0.5.2", output)
        self.assertIn("release_mode=reviewed-epic", output)
        self.assertIn("reviewed_pr=83", output)
        self.assertIn(
            "module-reviewed-epic-release.yml@" + REVIEWED_RELEASE_SHA,
            self.workflow,
        )
        self.assertIn("module-crate: tinysearch", self.workflow)
        self.assertIn("reviewed-pr: ${{ fromJSON(needs.resolve.outputs.reviewed_pr)", self.workflow)
        self.assertNotIn("module-crate:", self.workflow.split("module-reviewed-epic-release:", 1)[1])
        self.assertNotIn("asset-name:", self.workflow)
        self.assertNotIn("apt-packages:", self.workflow)

    def test_reviewed_release_rejects_missing_zero_and_noninteger_pr(self):
        for reviewed_pr in ("", "0", "-1", "1.5", "one"):
            with self.subTest(reviewed_pr=reviewed_pr):
                result, _ = self.run_resolve("reviewed-epic", reviewed_pr)
                self.assertNotEqual(result.returncode, 0)

    def test_reviewed_release_rejects_dispatch_outside_main(self):
        result, _ = self.run_resolve("reviewed-epic", "83", "refs/heads/feature")
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()

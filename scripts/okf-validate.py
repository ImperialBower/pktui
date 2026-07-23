#!/usr/bin/env python3
"""Minimal, dependency-free OKF v0.1 conformance checker.

Vendored so CI can verify the `.okf/` knowledge bundle without the okf
plugin/skill being installed. Implements the §9 hard rule only:

  1. Every non-reserved `.md` file has a parseable YAML frontmatter block
     (delimited by `---` on the first line and a closing `---`).
  2. Every such block has a non-empty `type` field.

Reserved files (`index.md`, `log.md`) are exempt, except the bundle-root
`index.md`, whose only permitted frontmatter key is `okf_version` — if it
carries frontmatter, we sanity-check that.

Exit code 0 = conformant, 1 = one or more errors, 2 = usage error.

Spec: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md
"""

from __future__ import annotations

import sys
from pathlib import Path

RESERVED = {"index.md", "log.md"}


def split_frontmatter(text: str) -> tuple[dict[str, str] | None, str]:
    """Return (frontmatter_dict, error). dict is None when absent/unparseable."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None, "missing opening '---' frontmatter delimiter"
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            fm: dict[str, str] = {}
            for raw in lines[1:i]:
                if not raw.strip() or raw.lstrip().startswith("#"):
                    continue
                # Only capture top-level `key: value` pairs (no nested keys).
                if raw[0] in " \t-":
                    continue
                if ":" not in raw:
                    return None, f"unparseable frontmatter line: {raw!r}"
                key, _, value = raw.partition(":")
                fm[key.strip()] = value.strip().strip("'\"")
            return fm, ""
    return None, "missing closing '---' frontmatter delimiter"


def check_bundle(root: Path) -> list[str]:
    errors: list[str] = []
    md_files = sorted(root.rglob("*.md"))
    if not md_files:
        return [f"no markdown files found under {root}"]

    for path in md_files:
        rel = path.relative_to(root)
        name = path.name
        text = path.read_text(encoding="utf-8")
        is_root_index = name == "index.md" and path.parent == root

        if name in RESERVED and not is_root_index:
            continue  # reserved files carry no frontmatter

        fm, err = split_frontmatter(text)

        if is_root_index:
            # Root index MAY carry frontmatter; if it does, only okf_version.
            if fm is not None:
                extra = [k for k in fm if k != "okf_version"]
                if extra:
                    errors.append(
                        f"{rel}: root index.md frontmatter may only contain "
                        f"'okf_version' (found: {', '.join(extra)})"
                    )
            continue

        if fm is None:
            errors.append(f"{rel}: {err}")
            continue
        if not fm.get("type"):
            errors.append(f"{rel}: frontmatter is missing a non-empty 'type' field")

    return errors


def main(argv: list[str]) -> int:
    args = [a for a in argv[1:] if a != "--strict"]  # --strict accepted for parity
    bundle = Path(args[0]) if args else Path(".okf")
    if not bundle.is_dir():
        print(f"error: bundle directory not found: {bundle}", file=sys.stderr)
        return 2

    errors = check_bundle(bundle)
    concepts = sum(
        1
        for p in bundle.rglob("*.md")
        if p.name not in RESERVED or (p.name == "index.md" and p.parent == bundle)
    )
    print(f"OKF v0.1 conformance — {bundle}  ({concepts} checked file(s))")
    if errors:
        for e in errors:
            print(f"  ERROR {e}")
        print(f"  ✗ {len(errors)} error(s)")
        return 1
    print("  ✓ conformant — no issues")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))

#!/usr/bin/env python3

import json
import os
import sys
import io
import re
import traceback
import datetime
from pathlib import Path

SETTINGS_FILE = ".claude/settings.json"
WATCHED_TOOLS = {'Write', 'Edit'}

__print = print
def print(*args, file=sys.stdout, **aargs) -> None:
    if isinstance(file, io.IOBase):
        __print(*args, file=file, **aargs)
        return

    for f in file:
        __print(*args, file=f, **aargs)

LOG_FILE = io.StringIO()

def flush_log() -> None:
    content = LOG_FILE.getvalue()
    if not content or len(sys.argv) < 2:
        return
    try:
        with open(sys.argv[1], "a", encoding="utf-8") as f:
            f.write(content)
    except OSError:
        pass

print('', sys.argv[0], datetime.datetime.now().isoformat(), sep='\n', file=LOG_FILE)


def main() -> None:

    (tool_name, path) = read_hook_input()

    project_dir = get_project_dir()

    print(f"tool: {tool_name}", file=LOG_FILE)
    print(f"path: {path}", file=LOG_FILE)
    print(f"project_dir: {project_dir}", file=LOG_FILE)

    # Check 1: Path must be within project directory
    try:
        path.relative_to(project_dir)
    except ValueError:
        print(
            f"blocked {tool_name} -- path '{path}' is outside project directory",
            file=[sys.stderr, LOG_FILE]
        )
        sys.exit(2)

    # Check 2: Path must NOT be within .claude/ directory
    claude_dir = project_dir / ".claude"
    try:
        path.relative_to(claude_dir)
        print(
            f"blocked {tool_name} -- path '{path}' is within protected .claude/ directory",
            file=[sys.stderr, LOG_FILE]
        )
        sys.exit(2)
    except ValueError:
        # Good - path is not in .claude/
        pass

    # Check 3: Path must match an allowed glob pattern from settings.json
    allowed_globs = load_allowed_globs(tool_name)

    print(f"allowed globs for {tool_name}:", file=LOG_FILE)
    print(*(g for g in allowed_globs), sep='\n', file=LOG_FILE)

    if not allowed_globs:
        print(
            f"blocked {tool_name} -- no allowed patterns found in settings.json",
            file=[sys.stderr, LOG_FILE]
        )
        sys.exit(2)

    # Get relative path from project directory for glob matching
    rel_path = path.relative_to(project_dir)

    print(f"relative path: {str(rel_path)}", file=LOG_FILE)

    for glob_pattern in allowed_globs:
        if rel_path.full_match(glob_pattern):
            print(f"allowed by pattern: {tool_name}({glob_pattern})", file=LOG_FILE)
            sys.exit(0)

    print(
        "blocked tool usage as it would fall outside the expressly permitted:\n"
        f"  {tool_name}({str(path)})",
        *(f"{tool_name}({glob})" for glob in allowed_globs),
        sep='\n - ',
        file=[sys.stderr, LOG_FILE]
    )
    sys.exit(2)


def load_allowed_globs(tool_name: str) -> list[str]:
    """Extract glob patterns for the given tool from settings.json allow list."""

    project_dir = get_project_dir()
    settings_file = project_dir / SETTINGS_FILE

    if not settings_file.exists():
        print(f"{SETTINGS_FILE} not found, blocking as precaution",
            file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    try:
        settings = json.loads(settings_file.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as e:
        print(f"failed to load {SETTINGS_FILE}: {e}, blocking as precaution",
            file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    allow_list = settings.get("permissions", {}).get("allow", [])

    if not isinstance(allow_list, list):
        print(f"allow list is not an array, blocking as precaution",
            file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    # Pattern to match "Write(glob)" or "Edit(glob)"
    pattern = re.compile(rf'^{re.escape(tool_name)}\((.+)\)$')

    globs = []
    for entry in allow_list:
        if not isinstance(entry, str):
            continue
        match = pattern.match(entry)
        if match:
            globs.append(match.group(1))

    return globs


def get_project_dir() -> Path:

    project_dir_env = os.environ.get("CLAUDE_PROJECT_DIR", "")

    if not project_dir_env:
        print("CLAUDE_PROJECT_DIR not set, blocking as precaution.",
            file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    try:
        return Path(project_dir_env).resolve()

    except ValueError:
        print(f"{project_dir_env} not resolvable, blocking as a precaution.",
            file=[sys.stderr, LOG_FILE])
        sys.exit(2)


def read_hook_input() -> tuple[str, Path]:
    hook_input = {}

    try:
        hook_input = json.load(sys.stdin)
        json.dump(hook_input, LOG_FILE, indent=1)
        LOG_FILE.write("\n")
    except (json.JSONDecodeError, ValueError):
        sys.exit(0)

    tool_name = hook_input.get("tool_name", "")
    if tool_name not in WATCHED_TOOLS:
        print(f"Not working on {tool_name}", file=LOG_FILE)
        sys.exit(0)

    tool_input = hook_input.get("tool_input", {})

    # Both Write and Edit use file_path
    path = tool_input.get("file_path")

    if not path:
        print("no file_path in tool input, blocking as precaution",
            file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    try:
        path = Path(path).resolve()
    except (ValueError, OSError):
        print(f"could not resolve path '{path}', blocking.",
            file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    return (tool_name, path)


if __name__ == "__main__":
    try:
        main()
    except SystemExit as e:
        print(f"sys.exit({e.code})", file=LOG_FILE)
        raise
    except Exception as e:
        traceback.print_exc(file=LOG_FILE)
        print("sys.exit(2)", file=LOG_FILE)
        print(repr(e), "caught, blocking as precaution")
        sys.exit(2)
    finally:
        flush_log()

#!/usr/bin/env python3
"""
Restricts Bash tool invocations to an explicit allowlist.
Deny by default - commands must match an allowed pattern to execute.
"""

import fnmatch
import json
import os
import sys
import io
import traceback
import datetime
from pathlib import Path

ALLOWED_COMMANDS_FILE = ".claude/bash-commands.json"
WATCHED_TOOLS = {'Bash'}

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
    command = read_hook_input()

    if command is None:
        # Not a Bash tool or parse error - allow (fail open for non-Bash)
        sys.exit(0)

    # Normalize command (strip leading/trailing whitespace)
    command = command.strip()
    print(f"command: {command!r}", file=LOG_FILE)

    allowed_patterns = load_allowed_commands()
    print(f"allowed patterns: {allowed_patterns}", file=LOG_FILE)

    if len(allowed_patterns) == 0:
        print("No allowed Bash command patterns configured, blocking all Bash tool usages",
              file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    for pattern in allowed_patterns:
        if fnmatch.fnmatchcase(command, pattern):
            check_command_integrity(command, pattern)
            print("allowed, matches", pattern, file=LOG_FILE)
            sys.exit(0)

        
    if "[*]" in command:
        print("Don't use a literal [*] -- that is a stand-in for a literal asterisk, use that instead", file=[sys.stderr, LOG_FILE])
    else:
        print(
            f"Blocked Bash({command}) -- it doesn't match the allowed patterns\n"
            "Allowed patterns:", *allowed_patterns,
            sep='\n - ',
            file=[sys.stderr, LOG_FILE]
        )
    print("Remember, you are running in the project root,",
        "there is no need for absolute paths.", file=[sys.stderr, LOG_FILE])
    sys.exit(2)

DANGEROUS = [
    '`', '(', '{', '\n', '>', '<', '&&', '&', ';', '||', '|', '(', '$', '$(', '${', '}', '?'
]

def check_command_integrity(command: str, pattern: str):

    reasons = []


    for danger in DANGEROUS:
        if command.count(danger) != pattern.count(danger):
            reasons.append(f"mismatch in uses of '{danger}' -- glob patterns cannot cover special characters")

    if command.count('*') != pattern.count('[*]'):
            reasons.append(f"mismatch in uses of '*' -- you can only use the glob stars exactly as they appear in the allowed patterns")

    if reasons:
        print(f"Blocking Bask({command}) because it doesn't fit the pattern {pattern}",
              *reasons,
              sep='\n - ',
              file=[sys.stderr, LOG_FILE])
        sys.exit(2)


def load_allowed_commands() -> list[str]:
    project_dir = get_project_dir()

    allowed_file = project_dir / ALLOWED_COMMANDS_FILE

    if not allowed_file.exists():
        print(f"{ALLOWED_COMMANDS_FILE} not found, blocking all Bash as precaution",
              file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    try:
        commands = json.loads(allowed_file.read_text(encoding='utf-8'))
        if isinstance(commands, list) and all(isinstance(c, str) for c in commands):
            return commands
    except (json.JSONDecodeError, OSError, ValueError):
        pass

    print(f"{ALLOWED_COMMANDS_FILE} not loaded, blocking all Bash as precaution",
          file=[sys.stderr, LOG_FILE])
    sys.exit(2)


def get_project_dir() -> Path:
    project_dir_env = os.environ.get("CLAUDE_PROJECT_DIR", "")

    if not project_dir_env:
        print("CLAUDE_PROJECT_DIR not set, blocking as precaution.",
              file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    try:
        return Path(project_dir_env).resolve()
    except ValueError:
        print(f"{project_dir_env} not resolvable, blocking as precaution.",
              file=[sys.stderr, LOG_FILE])
        sys.exit(2)


def read_hook_input() -> str | None:
    """Read and parse hook input, return command string or None if not applicable."""
    hook_input = {}

    try:
        hook_input = json.load(sys.stdin)
        json.dump(hook_input, LOG_FILE, indent=1)
        LOG_FILE.write("\n")
    except (json.JSONDecodeError, ValueError):
        return None

    tool_name = hook_input.get("tool_name", "")
    if tool_name not in WATCHED_TOOLS:
        print(f"Not watching {tool_name}", file=LOG_FILE)
        return None

    tool_input = hook_input.get("tool_input", {})
    command = tool_input.get("command", "")

    if not command:
        print("No command in Bash tool input", file=LOG_FILE)
        return None

    return command


if __name__ == '__main__':
    try:
        main()
    except SystemExit as e:
        print(f"sys.exit({e.code})", file=LOG_FILE)
        raise
    except Exception as e:
        traceback.print_exc(file=LOG_FILE)
        print(f"Exception caught, blocking as precaution: {e!r}",
              file=[sys.stderr, LOG_FILE])
        sys.exit(2)
    finally:
        flush_log()

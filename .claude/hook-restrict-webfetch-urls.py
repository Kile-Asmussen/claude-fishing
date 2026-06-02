#!/usr/bin/env python3

import json
import os
import sys
import io
import traceback
import datetime
from fnmatch import fnmatchcase
from pathlib import Path
from urllib.parse import urlsplit

WEBFETCH_URLS_FILE = ".claude/webfetch-urls.json"
WATCHED_TOOLS = { 'WebFetch' }

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

print('',  sys.argv[0], datetime.datetime.now().isoformat(), sep='\n', file=LOG_FILE)

def main() -> None:

    url = read_hook_input()
    print(url, file=LOG_FILE)

    parsed = urlsplit(url)
    print(repr(parsed), file=LOG_FILE)
    hostname = parsed.hostname or ""
    path = parsed.path or ""

    print(f"url: {url}", file=LOG_FILE)
    print(f"hostname: {hostname}", file=LOG_FILE)
    print(f"path: {path}", file=LOG_FILE)

    patterns = load_config()

    print("Url patterns:", *patterns, sep="\n", file=LOG_FILE)
    
    allowed = [p for p in patterns if not p.startswith('!')]
    exceptions = [p.removeprefix('!') for p in patterns if p.startswith('!')]

    reason: str = ''

    if not allowed:
        reason = "there are no allowed urls configured"

    allowed_by = next((
        pat for pat in allowed if fnmatch_url(hostname, path, pat)
    ), None)

    if not allowed_by:
        reason = reason or "it is not in allowed list of urls"

    denied_by = next((
        pat for pat in exceptions if fnmatch_url(hostname, path, pat)
    ), None)

    if denied_by:
        reason = reason or f"it matches the disallowed pattern '{denied_by}'"
    
    if not allowed_by or denied_by:
        print(f"Blocked WebFetch({hostname + path}) because {reason}.", file=[sys.stderr, LOG_FILE])
        if allowed:
            print(
                "Allowed urls take the form:" if any('*' in pat for pat in allowed) else "Allowed urls:",
                *allowed,
                sep='\n',
                file=[sys.stderr, LOG_FILE]
            )
        if exceptions:
            print(
                "Explicitly disallowed:",
                *exceptions,
                sep='\n',
                file=[sys.stderr, LOG_FILE]
            )
        
        print("Stop and ask the user for help if this is an important page to access.", file=[sys.stderr, LOG_FILE])
    
        sys.exit(2)

    print(
        f"Allowing WebFetch({hostname + path})\nhostname permitted by `{allowed_by}'",
        file=LOG_FILE
    )
    sys.exit(0)

def fnmatch_url(hostname: str, path: str, pattern: str) -> bool:
    if pattern.startswith('/'):
        print("looks like path-only pattern",repr(pattern), 'matching against', repr(path), file=LOG_FILE)
        return fnmatchcase(path, pattern)
    elif '/' not in pattern:
        print("looks like hostname-only pattern", repr(pattern), 'matching against', repr(hostname),  file=LOG_FILE)
        return fnmatchcase(hostname, pattern)
    elif '/' in pattern:
        print("looks like a mixed hostname-path pattern", repr(pattern), 'matching against', repr(hostname + path), file=LOG_FILE)
        return fnmatchcase(hostname + path, pattern)
    return False

def load_config() -> list[str]:

    project_dir = get_project_dir()

    config_file = project_dir / WEBFETCH_URLS_FILE

    if not config_file.exists():
        print(f"{WEBFETCH_URLS_FILE} not found, blocking as a precaution",
              file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    try:
        config = json.loads(config_file.read_text(encoding="utf-8"))
        
        if not isinstance(config, list) and all(isinstance(d, str) for d in config):
            raise ValueError("config must be a JSON list of strings")
        
        return config
        
    except (json.JSONDecodeError, OSError, ValueError) as e:
        print(f"{WEBFETCH_URLS_FILE} not loaded ({e}), blocking as a precaution",
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
        print(f"{project_dir_env} not resolvable, blocking as a precaution.",
              file=[sys.stderr, LOG_FILE])
        sys.exit(2)


def read_hook_input() -> str:
    hook_input = {}

    try:
        hook_input = json.load(sys.stdin)
        json.dump(hook_input, LOG_FILE, indent=1)
        LOG_FILE.write("\n")
    except (json.JSONDecodeError, ValueError):
        sys.exit(0)

    tool_name = hook_input.get("tool_name", "### NO_TOOL_NAME_GIVEN ###")
    if tool_name not in WATCHED_TOOLS:
        print(f"Not working on {tool_name}", file=LOG_FILE)
        sys.exit(0)

    tool_input = hook_input.get("tool_input", {})
    url = tool_input.get("url")

    if not url:
        print("No url in tool_input, blocking as a precaution.",
              file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    return url


if __name__ == '__main__':
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
#!/usr/bin/env python3


import json
import os
import sys
import io
import traceback
import datetime
from pathlib import Path

EXCEPTIONS_FILENAME = ".claude/read-grep-glob-paths.json"
WATCHED_TOOLS = { 'Read', 'Grep', 'Glob' }

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

    path = read_hook_input()
    
    print("path", str(path), file=LOG_FILE)
    
    allowed_paths = load_allowed_paths()
    
    print(f"allowing {len(allowed_paths)} paths", file=LOG_FILE)
    print(*(str(p) for p in allowed_paths), sep='\n', file=LOG_FILE)

    is_allowed = path in allowed_paths

    for allowed in allowed_paths:
        if is_allowed:
            break
        try:
            path.relative_to(allowed)
            is_allowed = True
        except ValueError:
            pass
    
    if not is_allowed:
        print(
            f"blocked access to disallowed path:\n"
            f"  {path}\n"
            "Allowed directories:",
            *allowed_paths,
            sep="\n - ",
            file=[sys.stderr, LOG_FILE]
        )
        sys.exit(2)
        
    sys.exit(0)

def load_allowed_paths() -> set[Path]:

    project_dir = get_project_dir()

    exceptions_file = project_dir / EXCEPTIONS_FILENAME

    if not exceptions_file.exists():
        return { project_dir }
    try:
        paths = json.loads(exceptions_file.read_text(encoding="utf-8"))
        if isinstance(paths, list) and all(isinstance(p, str) for p in paths):
            res = { Path(p).resolve() for p in paths }
            res.add(project_dir)
            return res
    except (json.JSONDecodeError, OSError, ValueError):
        pass

    return { project_dir }

def get_project_dir() -> Path:

    project_dir_env = os.environ.get("CLAUDE_PROJECT_DIR", "")

    if not project_dir_env:
        print("CLAUDE_PROJECT_DIR not set, blocking as precaution.", file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    try:
        return Path(project_dir_env).resolve()

    except ValueError:
        print(f"{project_dir_env} not resolvable, blocking as a precaution.", file=[sys.stderr, LOG_FILE])
        sys.exit(2)

def read_hook_input() -> Path:
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
    path = None

    # Read uses file_path; Glob and Grep use path (optional, defaults to cwd)
    if tool_name == "Read":
        path = tool_input.get("file_path")
    else:
        path = tool_input.get("path")

    if not path:
        path = Path.cwd()
    else:
        try:
            path = Path(path).resolve()
        except (ValueError, OSError):
            print(f"could not resolve path '{path}', blocking.", file=[sys.stderr, LOG_FILE])
            sys.exit(2)
    
    return path

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
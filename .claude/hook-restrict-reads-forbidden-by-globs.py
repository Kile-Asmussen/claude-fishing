#!/usr/bin/env python3


import json
import os
import sys
import io
import traceback
import datetime
from pathlib import Path

FORBIDDEN_GLOBS_FILE = ".claude/reads-forbidden-by-globs.json"
WATCHED_TOOLS = { 'Read', 'Grep' }

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

    (tool, path, glob, mode) = read_hook_input()
    
    glob = glob or "**/*"

    forbidden_globs = load_forbidden_globs()
    print("forbidden:", *forbidden_globs, file=LOG_FILE)
    

    if len(forbidden_globs) == 0:
        sys.exit(0)

    paths = []

    if tool == 'Read':
        paths = [path]

    if tool == 'Grep' and mode == 'content':
        paths = list(path.glob(glob))

    if not paths:
        sys.exit(0)

    print(f"inspecting {len(paths)} paths", file=LOG_FILE)
    print(*(str(p) for p in paths), sep='\n', file=LOG_FILE)

    for path in paths:

        try:
            path = path.resolve()
        except ValueError:
            continue

        for forbidden in forbidden_globs:
            if path.full_match(forbidden):
                print(
                    "blocking tool use -- it would read data from a forbidden file:\n"
                    f"  {str(path)}\n"
                    "Disallowed patterns:",
                    *forbidden_globs,
                    sep='\n - ',
                    file=[sys.stderr, LOG_FILE])
                sys.exit(2)
            
    sys.exit(0)

def load_forbidden_globs() -> set[str]:

    project_dir = get_project_dir()

    forbidden_globs_file = project_dir / FORBIDDEN_GLOBS_FILE

    if not forbidden_globs_file.exists():
        print(f"{str(FORBIDDEN_GLOBS_FILE)} was not found, stop and ask the user for help.",
            file=[sys.stderr, LOG_FILE])
        sys.exit(2)

    try:
        globs = json.loads(forbidden_globs_file.read_text(encoding="utf-8"))
        if isinstance(globs, list) and all(isinstance(g, str) for g in globs):
            return set(globs)
        else:
           print(f"{str(FORBIDDEN_GLOBS_FILE)} not an array of strings, stop and ask the user for help.", file=[sys.stderr, LOG_FILE])
    except json.JSONDecodeError:
        print(F"The {str(FORBIDDEN_GLOBS_FILE)} file is malformed JSON, stop and ask the user for help.", file=[sys.stderr, LOG_FILE])
    except OSError, ValueError:
       print(f"{str(FORBIDDEN_GLOBS_FILE)} not loaded correctly, stop and ask the user for help.", file=[sys.stderr, LOG_FILE])

 
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
            file=sys.stderr)
        sys.exit(2)


def read_hook_input() -> tuple[str, Path, str, str]:
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

    path = None
    glob = None
    mode = None

    # Read uses file_path; Glob and Grep use path (optional, defaults to cwd)
    if tool_name == "Read":
        path = tool_input.get("file_path")
    else:
        path = tool_input.get("path")
        glob = tool_input.get("glob")
        mode = tool_input.get("output_mode")

    if not path:
        path = Path.cwd()
    else:
        try:
            path = Path(path).resolve()
        except (ValueError, OSError):
            print(f"could not resolve path '{path}', blocking.",
            file=[sys.stderr, LOG_FILE])
            sys.exit(2)
    
    res = (tool_name, path, glob, mode)
    
    print(repr(res), file=LOG_FILE)
    
    return res

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
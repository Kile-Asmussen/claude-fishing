# To Do

## Field testing

- **Path normalization**: verify whether Claude Code sends file paths with or without a `./` prefix.
  The default paths config uses `./**/*` and `./*`, which require the `./` prefix to match.
  If Claude Code omits it, change the defaults to `**/*` and `*` (or normalize in `paths::check`).
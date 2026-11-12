# Static CLI root-help identity

This repository-authored ASCII asset is covered by the repository license. No
external font, runtime generator, image service or network asset is used. The
lowercase name is the existing executable identity, not a new release/product
name adoption. Ordinary semantic name, purpose, options and commands stay in help.

Only successful literal root `--help`/`-h` may decorate stdout. The binary-owned
helper requires a real stdout TTY, a known terminal family, a bounded declared
`COLUMNS` width of at least 72, and no `NO_COLOR` or `CLICOLOR=0`. Unknown width or
capabilities deliberately fall back to ordinary help. Width is a declaration,
not an ioctl/native terminal probe. No ANSI/color sequence is added. `NO_COLOR`
provides an undecorated screen-reader/high-contrast/copy-paste path.

Extra/global/machine flags, subcommand help, errors, version, completions,
reference generation and normal commands are never decorated. Assets remain in
the owning binary adapter; domain libraries do not expose art or emit it. There
is no shared sibling dependency or observability/logging change. Visual approval,
real TTY/pipe/platform conformance and product-name review remain pending.

Source checkpoint only: authored suppression cases were not executed. No tests,
build/compiler/lint/format/validator, CLI/SDK, native/provider/model/media, CI,
acquisition or publication ran. Flow #88 remains last and unchanged.

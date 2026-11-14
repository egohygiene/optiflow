# Flow #137 checkpoint 1 — November 13, 2026

Adds a bounded expression-only workflow lint job and `task ci:workflows` alias.
The local alias takes an explicit `ACTIONLINT` path and never downloads or
installs anything. The hosted job, when separately triggered, downloads the
pinned 1.7.12 Linux artifact and verifies its published SHA-256 before use.
The selected asset identity was read from the upstream release API; the binary
was not downloaded or executed during this source sprint.

The linter checks all owning-repository workflow YAML plus a valid step-level
runner-context fixture. It must also reject the deliberately invalid job-level
runner fixture outside `.github/workflows`. Both fixture expectations are
unexecuted. ShellCheck/Pyflakes and product compiler/tests remain separate lanes.
No heavy/native/model/corpus job is added to this five-minute, read-only job.
Its own path filters cover workflow/linter/fixture/task changes; concurrency
cancels superseded expression checks. A filtered workflow is not a universally
required branch-protection check without a separate status-policy decision.

The existing Aniflow corpus workflow moves `CORPUS_OUTPUT` to the qualification
step's environment, where `runner.temp` is allowed. Its manual/scheduled tiers,
maintainer opt-in gate, qualification command, output location and retention
are unchanged. The other suite repositories receive the same local expression
check independently; there is no sibling source or runtime dependency.

No formatter/linter, schema/validator, compiler/build, tests, native execution
or hosted workflow was run. This is an authored source repair, not a green
Actions result. The earlier execution deferral remains in force. Before marking
checkpoint 1 accepted, run the pinned lint/refusal cases and confirm each
workflow's declared selection without dispatching deferred qualification.

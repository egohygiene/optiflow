# Simulated volume faults

Relates to #93. Physical disconnect failures can be approximated at several
layers: kill the process to interrupt its journal sequence; force-detach a
disposable APFS image; inject read/write/sync errors through the existing test
ports; or use a disposable VM with a faulting virtual block device (for example
Linux dm-flakey) or virtual USB removal. These are useful distinct fault cases.
None independently witnesses a real drive/controller/bridge/cache losing power.

## Generated Mac image utility

`scripts/apfs_fault_trials.py` is an explicitly invoked native Mac test utility,
not production mutation dispatch. **It is authored source, unexecuted and
unqualified.** The current session remains Linux-only with native/test execution
deferred. Nothing on the maintainer's Mac was created, detached or modified.

Creation requires a fresh canonical absolute workspace under an existing parent.
It makes a 256 MiB APFS image (512 MiB is an explicit CLI option) and only
generated fixtures: `mount/collection/keeper.bin`, `candidate.bin`, `different.bin`
and a separate `mount/io-probe.bin`. The collection can later be selected for
separately reviewed native trials. Keep private media and trial state/logs
outside this utility's workspace. The utility accepts no arbitrary image path,
raw device, source directory, native Optiflow executable or provider argv.

After execution is separately resumed, use the explicit task interfaces:

```bash
task native:apfs:create WORKSPACE=/Users/YOUR_USER/optiflow-trial-UNIQUE
task native:apfs:disconnect WORKSPACE=/Users/YOUR_USER/optiflow-trial-UNIQUE TRIAL_SHA256=CREATE_RECORD_SHA256
task native:apfs:reconnect WORKSPACE=/Users/YOUR_USER/optiflow-trial-UNIQUE TRIAL_SHA256=CREATE_RECORD_SHA256 DISCONNECT_SHA256=DISCONNECT_RECORD_SHA256
task native:apfs:finish WORKSPACE=/Users/YOUR_USER/optiflow-trial-UNIQUE TRIAL_SHA256=CREATE_RECORD_SHA256 RECONNECT_SHA256=RECONNECT_RECORD_SHA256
```

Replace placeholders with actual returned digests. Each underlying CLI also
requires `--acknowledge-generated-volume-simulation`. `create` returns the exact
`trial.json` digest; subsequent phases return their own record digest. Intent
records are create-new and never replayed, even after failure or cancellation.
An uncertain trial requires manual investigation, not a rerun or adoption.

Before detach/reattach, the utility rechecks the private workspace/image
identity, external trial pin and current hdiutil image-to-mount association.
Foreign, aliased, ambiguous or changed objects refuse. Only the generated
workspace's mount point is passed to detach; no arbitrary device name is accepted.
Each fixed `/usr/bin/hdiutil` call has a 30-second direct-child timeout. Output is
privately spooled and metadata parsing is capped at 1 MiB per stream; this is
not a hard disk-output quota or proof that OS helpers stopped on timeout.
No native power-loss/full-sync durability claim is made by the local record flush.

The force-detach step holds the generated probe open and records the actual
post-detach read outcome. Cached reads may still return bytes; an I/O error is
never fabricated. Reconnect may fail or invoke filesystem recovery. Failure
preserves the image, partial records and intent; there is no recursive cleanup,
retry, mount adoption or original disposal. `finish` uses normal detachment and
retains the image and records for inspection.

## Evidence boundary

`optiflow.apfs-fault-observation.v1` always says
`evidence_kind: simulated_apfs_image_fault`,
`physical_disconnect_observed: false` and
`native_qualification_established: false`. It records native command-capture
digests and observed image attachment/descriptor behavior. It runs no Optiflow
scan/apply/restore/finalize, validates no transaction outcome and measures no
reclamation. Its records cannot substitute for native mutation command results
or a physical disconnect/reconnect row in the existing release reviewer.

Keep software fault coverage and real hardware trials as separate evidence.
#93's actual native mutation, three-target install/upgrade/rollback, physical
external-volume behavior and signed v0.2.0 release remain owed. #111's actual
backend/integration and #95's qualified replacement remain open; all production
qualification gates and the current v0.1.1 version remain unchanged.

Five focused source regressions are authored, uncompiled/unrun: owned-image
association, foreign/ambiguous mount refusal, non-replay of an intent, alias
refusal and an image-only packet retaining the missing physical-coverage finding.
No tests/builds/dependencies/validators, native/provider/media, tasks/CLI/SDK or
CI execution/polling occurred. Flow #88 remains last and untouched.

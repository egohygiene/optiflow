# Disposable Mac qualification workspace

Future operator preparation for #111/#93/#95; no command below was executed in
this Linux session. Existing check deferrals and production qualification gates
remain in place. A disk image supplies disposable software-test storage, not a
physical removable-drive disconnect witness or completed native qualification.

A MacBook can host small APFS disk images containing only generated fixtures.
Do not format an existing disk or point the tests at personal media. The example
creates one new 256 MiB image under a fresh temporary directory and keeps it for
inspection. Its variable/path never names a raw physical device:

```sh
opti_trial_root="$(mktemp -d "${TMPDIR:-/tmp}/optiflow-native.XXXXXXXX")"
mkdir "$opti_trial_root/mount"
hdiutil create -size 256m -fs APFS -volname OptiflowNativeTrial "$opti_trial_root/apfs.dmg"
hdiutil attach -nobrowse -mountpoint "$opti_trial_root/mount" "$opti_trial_root/apfs.dmg"
```

Record the exact image, mount, architecture, OS/filesystem and tool/source/build
identities before selecting generated fixtures or changing anything. A second
independently created image would supply a second filesystem for later reviewed
cross-volume checks. Current production Mac cross-volume dispatch still refuses
until its safe ACL backend, namespace/journal integration and native gates are
finished; creating an image does not enable them.

After the reviewed trial is stopped and all handles are closed, unmount this
explicit mount with `hdiutil detach "$opti_trial_root/mount"`. Preserve the image
and receipts when investigating a failure. Detach/reattach of a disk image is
software interruption coverage, not a physical disconnect, power-loss guarantee,
or permission to discard originals. No automatic recursive cleanup is provided.

The actual physical trial needs a separate spare drive holding disposable data,
with its exact identity and format selected by the operator. It must independently
record interruption, reconnect, recovery, finalization and free-space effects.
Never substitute the MacBook's internal disk or infer that a different drive is
empty. The MacBook covers its own native CPU target; the other Mac architecture
and Linux install/upgrade rows remain separate evidence requirements.

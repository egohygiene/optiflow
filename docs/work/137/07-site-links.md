# Flow #137 checkpoint 7 — public example links, November 13, 2026

The composed site copied documentation and schemas but omitted the root
`/examples/` files linked by seven documentation URLs. The existing link
checker correctly refused the incomplete site after generation.

The source repair adds an isolated public-example producer and reserves its
mount against landing-source collisions. Its single explicit allowlist contains
only the seven linked repository request/Rust files. It never recursively copies
`examples/`. Source reads refuse symlinks, hard-link aliases, nonregular files,
oversized or changed observations; the stage must be fresh under the existing
managed staging directory. Per-file/aggregate limits are 256 KiB/1 MiB.

The composed verifier requires exactly this reviewed download set, rejects extra
or aliased assets, and retains the existing broken-link gate. The site ownership
document records the new mount, review boundary and placeholder/unqualified
status of the examples. Adding another public download requires source review.

## Executed focused local checks

- `python3 -B -m unittest discover --start-directory tests --pattern test_site_examples.py -v`
  passed five synthetic cases: exact allowlisted bytes and excluded private
  canary; alias/oversize refusal before output creation; missing/extra/aliased
  mount refusal; aggregate-byte refusal with individually bounded files;
  landing mount collision retaining the prior `dist/` artifact.
- `bash -n scripts/site/build.sh` passed.
- `PATH=/workspace/.tools/optiflow-site-uv01133/bin:$PATH bash scripts/site/build.sh`
  completed the frozen Zensical build, architecture producer, composition and
  unchanged local-link gate. The seven intended downloads were present.

The initially installed uv 0.12.19 was rejected by the repository's existing
`>=0.11.33,<0.12` requirement. A task-private uv 0.11.33 was installed for the
successful run, preserving `pyproject.toml`/`uv.lock`. The local interpreter was
CPython 3.12.14, within the declared Python range; this is not CI's selected
Python 3.13 environment. No Rust product/compiler/tests, actual provider/native
qualification, GitHub Actions, deployment or publication ran.

The local build uses the existing explicit `--allow-missing-intelligence`
selection because no Intelligence producer was supplied. Production Pages
still requires its complete public Intelligence mount; this source check is
not live producer or deployment acceptance. The asset/link repair introduces no
new waiver. Other repair checkpoints, proposed decisions and #88 retain their
separate acceptance requirements.

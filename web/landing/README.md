# OptiFlow landing source

This directory contains the dependency-free static landing page published at
the root of `optiflow.egohygiene.io`. Its structure and responsive devtool
landing conventions are derived from [LaunchKit][launchkit] by Evil Martians,
then rewritten for OptiFlow's evidence-first product contract.

The upstream snapshot evaluated for this adaptation was commit
`b51f64e1bd88a01608c1561a2d3240f230de4f46`. The LaunchKit MIT license is
preserved in `LICENSE.launchkit`.

Do not write generated architecture or documentation into this directory. Run
`task site:build` to compose this source with the generated architecture portal
under `dist/architecture/` and the Zensical output under `dist/docs/`.

[launchkit]: https://github.com/evilmartians/devtool-template

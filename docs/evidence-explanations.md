# Optional immutable-evidence explanations (#133)

The first source checkpoint adds `explanation::prepare`, `validate_current` and a supplied fake candidate through existing SDK v1 `ReportProvider`. No runtime, model, Flow dependency, generic SDK/router or new configuration resolver is added. There is no new product CLI command in this checkpoint. Actual trusted local/remote profile, credential handoff, bounded provider invocation/usage reporting and model usefulness remain later #133 work.

A caller passes immutable report v6 bytes and an independently expected complete BLAKE3 digest from its existing artifact boundary, optionally a pinned review-only plan v5 of the same run/artifact set. Each input is at most4MiB, with at most16384 observations and4096 plan actions. Disabled preparation returns no packet without decoding inputs. Enabled preparation validates existing schemas, exact byte pins and report/plan linkage, then projects count-only facts. It reads no source file, SQLite state, environment, endpoint or model. It does not install/download, retry or fall back remotely.

Facts are explicitly RECORDED counts, not re-proven relationships: files/media/unsupported/unreadable, logical bytes, groups/duplicates, unstable observations, hard-link aliases, warnings and optional plan counts. The packet carries record schemas/digests/byte counts, with filenames, paths, source content, raw warning/metadata text and preconditions omitted. These record bytes are not authenticated, current filesystem observations or independently recomputed detector results. Historical schemas refuse here without changing ordinary report readers. The normal artifact-set reader remains responsible for marker/current artifact acceptance; this API does not invent it from a matching supplied digest.

A supplied candidate declares profile/template/output-schema digests and model/provider/runtime names. Those are untrusted declarations; the output-schema digest must match the exact candidate schema bytes. At most20 observed fact IDs are rendered as exact host values. Up to16 suggestions and16 questions have1..1024 UTF-8 bytes and1..8 unique packet fact IDs each. Unknown/duplicate references, changed records/pins/model identity and malformed shapes refuse or change output identity. `validate_current` reprojects caller-supplied CURRENT pinned record bytes; no current collection freshness is inferred. Semantic relevance/truth of suggestions remains UNVERIFIED despite valid citations.

Register `SuppliedExplanationProvider` with the existing extension registry using an explicit operator lock. Its capability must be SDK v1 report-provider, versioned, non-cacheable and partial-by-design, accepting `optiflow.explanation-packet.v1` and producing `optiflow.explanation-sidecar.v1`. Use `optiflow.evidence-explanation-config.v1` with closed `enabled` and exact optional `candidate_fingerprint` fields. The configuration is resolved/locked through existing SDK mechanisms; no environment is consulted. Pass exactly one fingerprinted normalized packet as `input_evidence` and its matching opaque immutable artifact reference (ID/schema/digest only), with no raw source artifacts, checkpoint references or granted effects. The host prepares that packet before invocation. This sidecar is a proposed report contribution; `ExtensionRegistry::invoke` still validates manifest/lock/invocation/provenance/result. Direct trait calls do not bypass that acceptance boundary.

Disabled/missing supplied candidates yield typed unavailable partial results with fixed redacted diagnostics; no model is attempted. Produced sidecars are partial by design and never plan/policy/validation contributions. Existing process providers retain cleared environments and no-secrets payloads. This implementation needs no credential integration; a real hosted/local runtime requires its own reviewed narrow trusted handoff rather than broad environment forwarding.

Prompt-like source names are omitted; note text remains inert untrusted plain text. Render without interpreting HTML/shell/Markdown links. Source bytes, plans and preconditions are untouched, and all eligibility/apply/physical-reclamation/provider-execution flags stay false. Advisory output cannot prove exact identity, quality, safe deletion, allocation savings or approval. No raw notes/prompts/paths go to portable telemetry; operational integration remains the existing Flow #14–#21 boundary, with no exporter here.

The synthetic walkthrough in authored cases is pinned report → normalized packet → fake candidate → cited report contribution. Ten source/schema/SDK cases are authored only, uncompiled/unrun. No live inference, private collection, candidate production or physical savings was observed. Full validation remains deferred to flow #88 last. ADR-0003 remains proposed; #133 remains open.

## Explicit pinned-record CLI

`explain prepare --report ABS --report-blake3 PIN [--plan ABS --plan-blake3 PIN]
--output ABS` saves only the existing normalized packet. `explain accept` takes
the same source pins plus `--packet ABS --candidate ABS --output ABS`, validates
current selected record bytes and publishes the existing separate advisory.
`explain inspect` takes those source/packet/candidate inputs and `--sidecar ABS`
to compare a saved advisory to a freshly recomputed result. Output files are new,
immutable records in existing private directories; no scan artifact member or
SDK role is redefined. Paths/names/raw media never enter the normalized packet.

Ordinary scans/plans/optimization never invoke explanations. These explicit
commands start no model or provider and read no credentials. Exact recorded
facts are separate from suggestion meaning; matching pins do not observe the
current collection, authorize mutation or prove physical space reclamation.
Three further CLI/staleness cases are authored, uncompiled and unrun.

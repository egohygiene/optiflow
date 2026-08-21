# Handle-Bound Observation Protocol

## Guarantee

On supported Linux and macOS filesystems, OptiFlow accepts file evidence only
when content, filesystem identity, and allocation metadata are derived from one
opened read-only file handle and the associated path remains bound to that
filesystem object for the complete observation window.

An accepted attempt performs these stages:

1. Capture a no-follow path signature after discovery.
2. Open the file once with read-only authority.
3. Capture the opened handle signature and compare it with the path signature.
4. Classify content, optionally run `ffprobe`, and calculate any required
   BLAKE3-256 hash through that handle.
5. Reinspect the handle, then reinspect the path without following symlinks.
6. Publish the attempt only if every required signature agrees.

The signature includes filesystem and file identifiers, logical size,
modification time, metadata-change time, and reported link count. Optional
`ffprobe` receives a clone of the opened handle through `/dev/fd/0`; it does not
resolve the original mutable pathname.

## Refusal and retry behavior

OptiFlow retries a failed observation at most once, for a total of two attempts.
Evidence from different attempts is never combined. If the path is replaced,
renamed away, truncated, extended, changed into a symlink, or has observable
metadata changed, the attempt's content hash, media evidence, identity, and
allocation facts are all rejected.

The final observation records a typed stability reason, non-current evidence
validity, the attempt count, and a warning. Unavailable identity or
metadata-change time fails closed: OptiFlow reports an unreadable observation
instead of making an exact-duplicate claim it cannot bind to an opened object.

Cancellation also publishes no evidence from the interrupted attempt. Opening
and reading source files does not grant write authority and the protocol does
not mutate source content or metadata.

## Cache binding

Reusable analysis is keyed by:

```text
native path + filesystem id + file id + size + modification time + metadata-change time
```

A cache entry is considered only inside a newly validated handle window. Cache
rows created before the full signature migration have missing identity fields
and cannot be reused. If the signature cannot be captured, cache lookup and
cache insertion are disabled for that observation.

## Filesystem limits

The protocol detects observable races; it does not claim that a portable
`stat` signature is an unforgeable object generation number. Inode reuse with
an exactly colliding size and timestamp signature is not universally
impossible, although including metadata-change time and holding the opened
handle materially narrows that risk. Network, virtual, or unusual filesystems
may also provide weak or unavailable identity and timestamp semantics. OptiFlow
refuses current evidence when the required fields are unavailable.

The final path check is still a point-in-time observation. A path may change
after the scan accepts it, so every future mutating workflow must re-open and
re-prove all plan preconditions at execution time.

## Verification map

| Invariant | Guard and evidence | Refusal or recovery |
| --- | --- | --- |
| One filesystem object per attempt | Pre-open path, opened-handle, final-handle, and final-path signatures | Discard the complete attempt |
| One handle for content evidence | In-process sniffing and hashing plus handle-fed `ffprobe` | Mark evidence unavailable or stale |
| No mixed retry evidence | Attempt-local analysis and metadata | Retry from an empty attempt, maximum two |
| Cache cannot bypass observation | Full signature key validated inside a fresh handle window | Miss or skip cache when identity is uncertain |
| Scanning remains read-only | `File::open` and inherited read-only descriptor | No mutation path exists |

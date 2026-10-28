---
title: Synthetic video input recipe
description: Unexecuted FFmpeg recipe for the first video candidate planning checkpoint.
---

# Synthetic video input recipe

This recipe prepares a tiny fake input for
[#146](https://github.com/egohygiene/optiflow/issues/146)'s later
inspect → candidate → compare walkthrough. The current checkpoint only
prepares a typed video plan. **There is no Optiflow video optimizer CLI yet.**
This recipe has not been executed or qualified; no video bytes, measured
properties or expected file hashes are checked in by this checkpoint.

At the deferred manual testing checkpoint, select a separately installed
**FFmpeg 8.0.1** build with the `lavfi` input, `testsrc2`, `setsar` and
`setfield` filters, FFV1 encoder and Matroska muxer. Replace the absolute path
below with that executable. Record its version/build and complete executable
digest with the eventual fixture evidence; the filename alone is not proof
of version. This repository does not install or download the provider.

```bash
(
  optiflow_ffmpeg="/absolute/path/to/ffmpeg-8.0.1"
  optiflow_demo_dir="$(mktemp -d "${TMPDIR:-/tmp}/optiflow-video-demo.XXXXXX")" || exit 1
  "$optiflow_ffmpeg" -hide_banner -nostdin -n \
    -f lavfi -i 'testsrc2=size=320x180:rate=25:duration=2' \
    -map 0:v:0 -an -sn -dn \
    -vf 'setsar=1,setfield=prog' \
    -c:v ffv1 -level:v 3 -threads:v 1 -pix_fmt yuv420p \
    -color_range tv -colorspace bt709 \
    -color_trc bt709 -color_primaries bt709 \
    -frames:v 50 -fps_mode passthrough \
    -map_metadata -1 -map_chapters -1 \
    -f matroska "$optiflow_demo_dir/source.mkv" &&
    printf '%s\n' "$optiflow_demo_dir/source.mkv"
)
```

The command creates a new disposable directory and uses `-n` to refuse an
existing output. Its only input is a generated test pattern. Metadata mapping
is disabled for this synthetic recipe; this is not permission to strip
metadata from user media. Keep any interrupted output in its disposable
directory until inspected; rerunning creates a different directory.

The intended input is one progressive, square-pixel 320×180 FFV1 video stream,
8-bit `yuv420p`, limited-range SDR BT.709, with 50 frames over two seconds at
25/1 fps. It has no intended audio, subtitles, attachments or chapters.
Matroska is expected to use a 1/1000 time base, starting at zero with 40-tick
frame spacing. These are recipe expectations: a future host observer must
verify the actual streams, decoded frame count, rational timing, color facts,
dispositions and all metadata before the input is eligible.

Generated encoder/duration bookkeeping tags may be present. The draft video
policy names those separately; descriptive or unknown metadata is unsupported.
Do not replace real observations with declarations from the example request.
Different builds can produce different container bytes and hashes even when
the intended image sequence is the same. No size reduction, decode result,
resource measurement or candidate validation is claimed by this recipe.

Reference: upstream [FFmpeg command options](https://ffmpeg.org/ffmpeg.html),
[test-source and video filters](https://ffmpeg.org/ffmpeg-filters.html), and
[codec options](https://ffmpeg.org/ffmpeg-codecs.html). Those current manuals
explain the option intent; native FFmpeg 8.0.1 qualification remains deferred.

# Plan 02b — system-font feasibility probe

> **For agentic workers:** REQUIRED SUB-SKILL: executing-plans-strict. Steps use `- [ ]`.

**Goal:** find out whether a system font improves tray legibility at the actual GNOME panel size
before selecting a renderer for the CPU and Memory items.

**Architecture:** run one temporary Rust tray item at a time, with identical 64×16 canvases and
fixed-width CPU labels in both modes. The control uses the S1 bitmap renderer. The trial uses
three pre-rasterised system-font frames in the same tray process. Measure both tray processes at
tick 15. The probe cannot measure the Pss of a production font stack because rasterisation happens
before launch.

**Tech stack:** the locked [S1 crate](../../spikes/tray/Cargo.toml), installed Pillow with FreeType,
Fontconfig's installed bold sans-serif face, GNOME's screenshot portal, and the existing
`measure.sh`.

**Branch:** `feature/tray-spikes`. Keep the existing staged 02a files intact. Probe code runs in a
temporary copy without editing the staged crate. Plan the daemon on its own branch once the
renderer and thread invariant are settled.

## Confirmed Requirements

- Trial the system font with one tray item, at the same scale and in both styles as S1.
- Keep 7%, 37%, and 100% right-aligned in the same field, with the CPU glyph retained.
- Compare native-size evidence and Pss. Record the font, dimensions, and the limits of the results.
- The production renderer decision and daemon thread invariant are separate decisions.

## Global Constraints

- Code and docs in English. No dependency or product API changes. Keep the core pure.
- Bound external calls and release the tray item on exit. Restore the original appearance.
- Run format, Clippy, tests, workspace verification and an end-to-end GNOME check.
- Keep screenshots outside Git. Commit or push only when requested.

## Technical Defaults

- Compare one font at the same dimensions as S1 variant A to isolate the effect of the font.
- Render white Noto Sans Bold at 18 px into the rightmost 47 columns, leaving the glyph untouched.
- Treat the settings of this GNOME session as fixed for both modes.

## Risks & Rollback

| Risk | Mitigation | Rollback |
|---|---|---|
| GNOME scales both pixmaps to the same small slot | Inspect the native-size captures in both styles | Close the probe and retain the measured result |
| The appearance or tray item persists | Restore appearance and terminate/reap the process with a shell exit trap | Restore the pre-run style and stop the recorded PID |
| The Pss comparison is mistaken for the rasteriser cost | Record that Pillow exits before either measurement | Measure a runtime rasteriser separately if the font passes |
| Existing 02a changes are staged | Use a temporary copy and compare the staged diff before and after | Keep 02a staging intact |

## Task 1 — Compare the bitmap and font

**Files:** temporary `make-frames.py` and `src/bin/font_probe.rs` in a copy of `spikes/tray`.
**Consumes:** `statsbar_tray_spike::render`, `bounded`, and `measure.sh <pid>`.
**Produces:** one tray item cycling 7/37/100%, native-size crops, and Pss in both modes.

- [x] Test that `compose(64×16 base, 47×16 overlay)` preserves the glyph, changes only the
  text field, and rejects a frame of the wrong length. Run `cargo test --offline --locked --release --bin
  font_probe` in the temporary crate and expect it to fail until `compose` exists.
- [x] Render the three overlays from the system face. Align on the right ink edge and use a
  shared vertical origin. Emit 4 bytes per pixel in ARGB order. Compose the frames onto the
  transparent text field of the S1 CPU glyph. Run the composition tests, the release build,
  Clippy and formatting.
- [x] Run the same binary in both modes. Capture matched values in Dark and Light through the
  screenshot portal in each mode. Measure each process after tick 15 and restore the original
  style. Verify that the watcher removed each item.

## Task 2 — Verification and evidence

**Files:** `docs/spikes/2026-11-18-02b-system-font.md` and this plan.
**Consumes:** screenshots at native size, printed tick logs and `measure.sh` output.
**Produces:** recorded outcome for the renderer decision.

- [x] Record the observed height and legibility in both styles and the Pss for both tray
  processes. Report whether evidence supports selecting this renderer, and any unmeasured cost.
- [x] Run `./scripts/check.sh` and verify that `git diff --cached --binary` is unchanged.
  Review the entire experiment, including cleanup, with `code-review-strict`.
- [x] Ready to commit the probe plan and results when requested.

## Temporary probe sources

Start with the test module below as `src/bin/font_probe.rs`. After observing the compiler
error for `compose`, prepend the Rust implementation, then save the Python generator as
`make-frames.py`. Run the generator with `python3 make-frames.py "$font" frames`, where
`font` is obtained with `fc-match -f '%{file}' 'sans-serif:style=Bold'`.

### Composition tests

```rust
#[cfg(test)]
mod tests {
    use super::compose;
    use statsbar_tray_spike::{Glyph, render};

    #[test]
    fn changes_the_text_field_without_touching_the_glyph() {
        let glyph = render(Glyph::Cpu, "", 2, 16, [255; 4]);
        let original = glyph.data.clone();
        let overlay = vec![220; 47 * 16 * 4];
        let composed = compose(glyph, &overlay).unwrap();

        assert_eq!((composed.width, composed.height), (64, 16));
        for (row, old_row) in composed
            .data
            .as_chunks::<{ 64 * 4 }>()
            .0
            .iter()
            .zip(original.as_chunks::<{ 64 * 4 }>().0)
        {
            assert_eq!(&row[..17 * 4], &old_row[..17 * 4]);
            assert_eq!(&row[17 * 4..], &[220; 47 * 4]);
        }
    }

    #[test]
    fn rejects_a_truncated_overlay() {
        let glyph = render(Glyph::Cpu, "", 2, 16, [255; 4]);
        let error = compose(glyph, &vec![0; 47 * 16 * 4 - 1]).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
}
```

### Rust implementation

```rust
use std::{error::Error, io, path::Path, time::Duration};

use futures_lite::StreamExt;
use ksni::{Icon, Tray, TrayMethods};
use statsbar_tray_spike::{Glyph, SETUP_TIMEOUT, bounded, render};

const WIDTH: usize = 64;
const HEIGHT: usize = 16;
const FIELD: usize = 47;
const PIXEL_BYTES: usize = 4;
const VALUES: [&str; 3] = ["7%", "37%", "100%"];

fn compose(mut base: Icon, overlay: &[u8]) -> io::Result<Icon> {
    let overlay_stride = FIELD * PIXEL_BYTES;
    if overlay.len() != overlay_stride * HEIGHT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wrong overlay byte count",
        ));
    }
    for (target, source) in base
        .data
        .as_chunks_mut::<{ WIDTH * PIXEL_BYTES }>()
        .0
        .iter_mut()
        .zip(overlay.as_chunks::<{ FIELD * PIXEL_BYTES }>().0)
    {
        target[(WIDTH - FIELD) * PIXEL_BYTES..].copy_from_slice(source);
    }
    Ok(base)
}

struct Trial {
    frames: [Icon; 3],
    index: usize,
}

impl Tray for Trial {
    fn id(&self) -> String {
        "statsbar-font-probe".into()
    }

    fn title(&self) -> String {
        format!("StatsBar font probe: {}", VALUES[self.index])
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![self.frames[self.index].clone()]
    }
}

fn frame(value: &str, directory: Option<&Path>) -> Result<Icon, Box<dyn Error>> {
    match directory {
        None => Ok(render(Glyph::Cpu, value, 2, HEIGHT, [255; 4])),
        Some(directory) => {
            let overlay = std::fs::read(directory.join(format!("{value}.argb")))?;
            Ok(compose(
                render(Glyph::Cpu, "", 2, HEIGHT, [255; 4]),
                &overlay,
            )?)
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory = args.next();
    if args.next().is_some() {
        return Err("usage: font_probe [overlay_directory]".into());
    }
    let directory = directory.as_deref().map(Path::new);
    let frames = [
        frame(VALUES[0], directory)?,
        frame(VALUES[1], directory)?,
        frame(VALUES[2], directory)?,
    ];
    async_io::block_on(async {
        let handle = bounded(
            "tray registration",
            SETUP_TIMEOUT,
            Trial { frames, index: 0 }.spawn(),
        )
        .await??;
        println!("pid {}: ready", std::process::id());
        let mut timer = async_io::Timer::interval(Duration::from_secs(2));
        let mut tick = 0_usize;
        loop {
            timer.next().await;
            tick = tick.wrapping_add(1);
            let index = tick % VALUES.len();
            bounded(
                "tray update",
                SETUP_TIMEOUT,
                handle.update(|item| item.index = index),
            )
            .await?
            .ok_or("font probe tray service shut down")?;
            println!("tick {tick}: {}", VALUES[index]);
        }
    })
}
```

### Font overlay generator

```python
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

font = ImageFont.truetype(sys.argv[1], 18)
output = Path(sys.argv[2])
output.mkdir(exist_ok=True)
for value in ("7%", "37%", "100%"):
    shade = Image.new("L", (47, 16))
    left, top, right, bottom = font.getbbox(value)
    if right - left > 47 or top < 5 or bottom > 21:
        raise ValueError(f"Label does not fit: {value}")
    ImageDraw.Draw(shade).text((47 - right, -5), value, font=font, fill=255)
    ink = shade.getbbox()
    if ink is None:
        raise ValueError(f"Label has no ink: {value}")
    ink_left, _, ink_right, _ = ink
    tight = shade.crop((ink_left, 0, ink_right, 16))
    shade = Image.new("L", (47, 16))
    shade.paste(tight, (47 - tight.width, 0))
    pixels = bytearray()
    for alpha in shade.tobytes():
        pixels.extend((alpha, 255, 255, 255) if alpha else (0, 0, 0, 0))
    (output / f"{value}.argb").write_bytes(pixels)
```

### Build and run

In the temporary copy, run the following after composing the Rust file and generating
`frames/`. Point `CARGO_TARGET_DIR` to the existing spike target to reuse the lockfile build.

```bash
cargo fmt
cargo test --offline --locked --release --bin font_probe
cargo build --offline --locked --release --bin font_probe
cargo clippy --offline --locked --release --all-targets -- -D warnings
cargo fmt --check
```

Save the following as `observe.sh` in the temporary directory. Save the screenshot helper below as
`capture.py`. Run
`timeout --kill-after=5s 140s bash observe.sh "$probe_dir" "$probe_binary" "$measure"`
from the active GNOME session. Keep the captured desktop pixels outside Git.

```bash
#!/usr/bin/env bash
set -euo pipefail
probe_dir="$1"
probe_binary="$2"
measure="$3"
appearance='org.gnome.desktop.interface'
original="$(timeout 5 gsettings get "$appearance" color-scheme)"
original="$(printf '%s' "$original" | tr -d "'")"
pid=

stop_probe() {
    if [[ -n "$pid" ]]; then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    fi
}

restore() {
    stop_probe
    timeout 5 gsettings set "$appearance" color-scheme "$original"
}
trap restore EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

watcher() {
    timeout 5 gdbus call --session --dest org.kde.StatusNotifierWatcher \
        --object-path /StatusNotifierWatcher \
        --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher \
        RegisteredStatusNotifierItems
}

wait_for_tick() {
    timeout "$1" bash -c '
        until grep -qE "$1" "$2"; do
            kill -0 "$3" 2>/dev/null || exit 1
            sleep 0.1
        done
    ' bash "$2" "$log" "$pid" || { cat "$log" >&2; return 1; }
}

for mode in bitmap font; do
    args=()
    if [[ "$mode" = font ]]; then
        args+=("$probe_dir/frames")
    fi
    timeout 5 gsettings set "$appearance" color-scheme prefer-dark
    log="$probe_dir/$mode.log"
    "$probe_binary" "${args[@]}" > "$log" 2>&1 &
    pid=$!
    wait_for_tick 10 '^tick 1:'
    before="$(watcher)"
    [[ "$before" == *"org.kde.StatusNotifierItem-$pid-"* ]]
    for style in dark light; do
        if [[ "$style" = light ]]; then
            timeout 5 gsettings set "$appearance" color-scheme default
            sleep 0.8
        fi
        for label in 7 37 100; do
            seen="$(wc -l < "$log")"
            timeout 8 bash -c '
                until tail -n "+$1" "$2" | grep -qE "^tick [0-9]+: $3%$"; do
                    kill -0 "$4" 2>/dev/null || exit 1
                    sleep 0.1
                done
            ' bash "$((seen + 1))" "$log" "$label" "$pid"
            sleep 0.22
            timeout 12 /usr/bin/python3 "$probe_dir/capture.py" "$probe_dir/$mode-$style-$label.png"
            [[ "$(tail -1 "$log")" == *": $label%" ]]
        done
    done
    wait_for_tick 45 '^tick 15:'
    "$measure" "$pid" | tee "$probe_dir/$mode-measure.txt"
    stopped_pid="$pid"
    stop_probe
    pid=
    for attempt in 1 2 3 4 5; do
        after="$(watcher)"
        [[ "$after" != *"org.kde.StatusNotifierItem-$stopped_pid-"* ]] && break
        sleep 0.2
    done
    [[ "$after" != *"org.kde.StatusNotifierItem-$stopped_pid-"* ]]
    printf '%s tray item removed\n' "$mode"
done
```

### GNOME screenshot helper

```python
import sys
from pathlib import Path
from uuid import uuid4

import gi

gi.require_version('GdkPixbuf', '2.0')
from gi.repository import Gio, GLib, GdkPixbuf

output = Path(sys.argv[1])
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
sender = bus.get_unique_name()[1:].replace('.', '_')
token = 'statsbar_' + uuid4().hex
request = f'/org/freedesktop/portal/desktop/request/{sender}/{token}'
loop = GLib.MainLoop()
result = 1

def response(_connection, _sender, _path, _interface, _signal, values):
    global result
    try:
        code, data = values.unpack()
        if code:
            print(f'GNOME screenshot portal returned {code}')
            return
        filename = Path(Gio.File.new_for_uri(data['uri']).get_path())
        try:
            image = GdkPixbuf.Pixbuf.new_from_file(str(filename))
            panel = image.new_subpixbuf(image.get_width() - 515, 10, 240, 20)
            panel.savev(str(output), 'png', [], [])
        finally:
            filename.unlink()
        print(output)
        result = 0
    finally:
        loop.quit()

bus.signal_subscribe('org.freedesktop.portal.Desktop', 'org.freedesktop.portal.Request', 'Response', request, None, Gio.DBusSignalFlags.NONE, response)
options = {'handle_token': GLib.Variant('s', token), 'interactive': GLib.Variant('b', False)}
reply = bus.call_sync('org.freedesktop.portal.Desktop', '/org/freedesktop/portal/desktop', 'org.freedesktop.portal.Screenshot', 'Screenshot', GLib.Variant('(sa{sv})', ('', options)), GLib.VariantType.new('(o)'), Gio.DBusCallFlags.NONE, 5000, None)
if reply.unpack()[0] != request:
    raise SystemExit('GNOME screenshot portal returned an unexpected request path')
loop.run()
raise SystemExit(result)
```

## Follow-up — square pixmap

The user chose to keep the percentage visible and follow
[Stats' Mini widget](https://github.com/exelban/stats/blob/master/Kit/Widgets/Mini.swift), which
places a small module label above the larger value. Try a 20×20 white-on-transparent pixmap with
a small `CPU` label and a compact nine-row percentage beneath it. Keep a fixed width and align
both lines on the right. This probe changes the font and the layout together and compares the
combined result with the earlier 64×16 control.

- [x] In a second temporary copy, add tests for a square frame's dimensions and payload and reject
  incorrect byte counts. Run the release test before and after adding the square decoder.
- [x] Generate the 7%, 37%, and 100% square frames and check their bounds. Reuse the observation
  runner for a single square-mode process. Capture Dark and Light at native size and measure Pss
  after tick 15. Verify the watcher removes the item and the original appearance returns.
- [x] Compare the result with the previous control, repeat workspace verification and review, and
  record the limits of selecting a production renderer.

Use the following changes on a fresh copy of the font probe. For the initial failing run,
add the two new tests first and run the same release test command. After applying the full
changes, run the same format, tests, build, Clippy and observation commands.

```diff
--- a/src/bin/font_probe.rs
+++ b/src/bin/font_probe.rs
@@ -7,6 +7,7 @@
 const WIDTH: usize = 64;
 const HEIGHT: usize = 16;
 const FIELD: usize = 47;
+const SQUARE_SIZE: i32 = 20;
 const PIXEL_BYTES: usize = 4;
 const VALUES: [&str; 3] = ["7%", "37%", "100%"];
 
@@ -30,6 +31,20 @@
     Ok(base)
 }
 
+fn square_frame(data: Vec<u8>) -> io::Result<Icon> {
+    if data.len() != SQUARE_SIZE as usize * SQUARE_SIZE as usize * PIXEL_BYTES {
+        return Err(io::Error::new(
+            io::ErrorKind::InvalidData,
+            "wrong square-frame byte count",
+        ));
+    }
+    Ok(Icon {
+        width: SQUARE_SIZE,
+        height: SQUARE_SIZE,
+        data,
+    })
+}
+
 struct Trial {
     frames: [Icon; 3],
     index: usize,
@@ -49,11 +64,14 @@
     }
 }
 
-fn frame(value: &str, directory: Option<&Path>) -> Result<Icon, Box<dyn Error>> {
+fn frame(value: &str, directory: Option<&Path>, square_mode: bool) -> Result<Icon, Box<dyn Error>> {
     match directory {
         None => Ok(render(Glyph::Cpu, value, 2, HEIGHT, [255; 4])),
         Some(directory) => {
             let overlay = std::fs::read(directory.join(format!("{value}.argb")))?;
+            if square_mode {
+                return Ok(square_frame(overlay)?);
+            }
             Ok(compose(
                 render(Glyph::Cpu, "", 2, HEIGHT, [255; 4]),
                 &overlay,
@@ -63,16 +81,17 @@
 }
 
 fn main() -> Result<(), Box<dyn Error>> {
-    let mut args = std::env::args_os().skip(1);
+    let mut args = std::env::args_os().skip(1).peekable();
+    let square_mode = args.next_if(|arg| arg == "--square").is_some();
     let directory = args.next();
-    if args.next().is_some() {
-        return Err("usage: font_probe [overlay_directory]".into());
+    if (square_mode && directory.is_none()) || args.next().is_some() {
+        return Err("usage: font_probe [overlay_directory | --square frame_directory]".into());
     }
     let directory = directory.as_deref().map(Path::new);
     let frames = [
-        frame(VALUES[0], directory)?,
-        frame(VALUES[1], directory)?,
-        frame(VALUES[2], directory)?,
+        frame(VALUES[0], directory, square_mode)?,
+        frame(VALUES[1], directory, square_mode)?,
+        frame(VALUES[2], directory, square_mode)?,
     ];
     async_io::block_on(async {
         let handle = bounded(
@@ -131,4 +150,18 @@
         let error = compose(glyph, &vec![0; 47 * 16 * 4 - 1]).unwrap_err();
         assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
     }
+
+    #[test]
+    fn decodes_square_pixmap_with_exact_dimensions_and_payload() {
+        let bytes = vec![220; 20 * 20 * 4];
+        let frame = super::square_frame(bytes.clone()).unwrap();
+        assert_eq!((frame.width, frame.height), (20, 20));
+        assert_eq!(frame.data, bytes);
+    }
+
+    #[test]
+    fn rejects_a_square_pixmap_with_an_extra_byte() {
+        let result = super::square_frame(vec![0; 20 * 20 * 4 + 1]);
+        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
+    }
 }
--- a/observe.sh
+++ b/observe.sh
@@ -39,10 +39,10 @@
     ' bash "$2" "$log" "$pid" || { cat "$log" >&2; return 1; }
 }
 
-for mode in bitmap font; do
+for mode in square; do
     args=()
-    if [[ "$mode" = font ]]; then
-        args+=("$probe_dir/frames")
+    if [[ "$mode" = square ]]; then
+        args+=(--square "$probe_dir/frames")
     fi
     timeout 5 gsettings set "$appearance" color-scheme prefer-dark
     log="$probe_dir/$mode.log"
--- a/capture.py
+++ b/capture.py
@@ -25,7 +25,7 @@
         filename = Path(Gio.File.new_for_uri(data['uri']).get_path())
         try:
             image = GdkPixbuf.Pixbuf.new_from_file(str(filename))
-            panel = image.new_subpixbuf(image.get_width() - 515, 10, 240, 20)
+            panel = image.new_subpixbuf(image.get_width() - 515, 6, 240, 29)
             panel.savev(str(output), 'png', [], [])
         finally:
             filename.unlink()
```

Save the following as `make-square.py` and run `python3 make-square.py frames` before observation.

```python
import sys
from pathlib import Path

SIZE = 20
VALUES = ("7%", "37%", "100%")
LABEL = {
    "C": (".##", "#..", "#..", "#..", ".##"),
    "P": ("##.", "#.#", "##.", "#..", "#.."),
    "U": ("#.#", "#.#", "#.#", "#.#", ".#."),
}
DIGITS = {
    "0": (".#.", "#.#", "#.#", "#.#", "#.#", "#.#", "#.#", "#.#", ".#."),
    "1": (".#.", "##.", ".#.", ".#.", ".#.", ".#.", ".#.", ".#.", "###"),
    "3": ("##.", "..#", "..#", "..#", ".##", "..#", "..#", "..#", "##."),
    "7": ("###", "..#", "..#", "..#", ".#.", ".#.", "#..", "#..", "#.."),
    "%": ("##..#", "##..#", "...#.", "...#.", "..#..", ".#...", ".#.##", "#..##", "#...."),
}

def draw(canvas, value, glyphs, top):
    width = sum(len(glyphs[character][0]) + 1 for character in value) - 1
    left = SIZE - 1 - width
    for character in value:
        glyph = glyphs[character]
        for row, line in enumerate(glyph):
            for col, pixel in enumerate(line):
                if pixel == "#":
                    offset = ((top + row) * SIZE + left + col) * 4
                    canvas[offset:offset + 4] = b"\xff" * 4
        left += len(glyph[0]) + 1

output = Path(sys.argv[1])
output.mkdir(exist_ok=True)
for value in VALUES:
    canvas = bytearray(SIZE * SIZE * 4)
    draw(canvas, "CPU", LABEL, 2)
    draw(canvas, value, DIGITS, 10)
    (output / f"{value}.argb").write_bytes(canvas)
```

# Plan 02b probe — system font in the GNOME tray

Run on 2026-11-18 on Ubuntu 26.04.1, GNOME Shell 50.1, Wayland, 125% display scale.
The test plan and runnable probe sources are in [the font probe plan](../plans/2026-11-18-rust-statsbar-02b-font-probe.md).

One tray item at a time offered a 64×16 pixmap with the S1 CPU glyph and a fixed-width field cycling
through 7%, 37%, and 100%. The same executable used either the S1 bitmap font or an 18 px Noto Sans
Bold face selected by Fontconfig and rasterised with Pillow/FreeType before launch.

| Mode | Visible text height, Dark / Light | Visible glyph and label width | Rss at tick 15 | Pss at tick 15 | Threads |
|---|---|---|---|---|---|
| S1 bitmap | 6 px / 6 px | 21 px | 4112 kB | 2163 kB | 4 |
| Noto Sans Bold | 5 px / 5 px | 21 px | 4124 kB | 2175 kB | 4 |

The crops for each value kept the same right edge. The GNOME panel background was `#131313` in both
styles. The measured bounds count pixels with a channel at least eight levels away from that
background. At native size the percentage was hard to read in both modes, especially with
`100%`. The system font did not solve the readability problem at these dimensions.

The Pss difference was 12 kB in one 30 s run per mode. These two samples measure the tray process
with its prepared frames. They do not measure an in-process rasteriser or the cost of loading fonts
in the daemon. The result applies to the tested font, pixmap geometry and GNOME setup.

The watcher removed both test items after their processes exited. The original Dark appearance was
restored by the executor, which also made the style switches, with the user's approval. The
comparison image is kept outside Git.

## Stats Mini layout

The user chose the [Stats Mini layout](https://github.com/exelban/stats/blob/master/Kit/Widgets/Mini.swift).
The GNOME adaptation uses a 20×20 pixmap with a small `CPU` label over a larger percentage. Both
lines are right-aligned. The prototype uses compact monochrome bitmap characters.

| Mode | Visible label height, Dark / Light | Visible percentage height, Dark / Light | Rss at tick 15 | Pss at tick 15 | Threads |
|---|---|---|---|---|---|
| 20×20 Mini layout | 7 px / 7 px | 11 px / 11 px | 3940 kB | 1991 kB | 4 |

At native screenshot size the three percentages were easier to distinguish than the 64×16 variants.
In all six Mini captures the percentage ended at the same right edge. The foreground and background
matched between styles. GNOME removed the item, and the executor restored Dark after the run.

The square layout changed the pixmap shape, typography and label placement together. The single
30 s Pss sample is a probe reading, so it cannot isolate a saving from the geometry. This run
covers the CPU label, three percentages and one display scale.

**Decision:** use the Stats Mini arrangement as the reference for Plan 02b. The daemon needs a
renderer that handles the full 0–100% range and a separate live check for RAM.

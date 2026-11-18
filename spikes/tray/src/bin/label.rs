//! Spike S1: does a wide pixmap stay wide and legible in the GNOME panel?
//!
//! Shows five items side by side. Look at each in the panel, with the light and the dark
//! style, at the display's normal scale, and record what you see in docs/spikes/.

use std::time::Duration;

use futures_lite::StreamExt;
use ksni::TrayMethods;
use statsbar_tray_spike::{Glyph, SETUP_TIMEOUT, bounded, render};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [255, 0, 0, 0];

struct Variant {
    /// Single letter shown in the item's title and tooltip, so a screenshot can name it.
    name: &'static str,
    glyph: Glyph,
    text: String,
    /// Pixmaps offered to the host, as (dot size, height).
    sizes: Vec<(usize, usize)>,
    argb: [u8; 4],
}

impl ksni::Tray for Variant {
    fn id(&self) -> String {
        format!("statsbar-spike-label-{}", self.name)
    }
    fn title(&self) -> String {
        format!("Variant {}", self.name)
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: format!("Variant {}", self.name),
            ..Default::default()
        }
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.sizes
            .iter()
            .map(|&(dot, height)| render(self.glyph, &self.text, dot, height, self.argb))
            .collect()
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::menu::StandardItem {
                label: format!("Variant {}: quit spike", self.name),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let variants = vec![
        // A: the candidate design, 16 px tall, cycling so the redraw and the fixed width show.
        Variant {
            name: "A",
            glyph: Glyph::Cpu,
            text: "7%".into(),
            sizes: vec![(2, 16)],
            argb: WHITE,
        },
        // B: the same drawing at twice the pixels, for a host that wants a larger pixmap.
        Variant {
            name: "B",
            glyph: Glyph::Memory,
            text: "100%".into(),
            sizes: vec![(4, 32)],
            argb: WHITE,
        },
        // C: both sizes offered, letting the host choose.
        Variant {
            name: "C",
            glyph: Glyph::Cpu,
            text: "100%".into(),
            sizes: vec![(2, 16), (4, 32)],
            argb: WHITE,
        },
        // D: dark dots, for a light panel.
        Variant {
            name: "D",
            glyph: Glyph::Memory,
            text: "100%".into(),
            sizes: vec![(2, 16)],
            argb: BLACK,
        },
        // E: one dot per pixel, the smallest the font can be.
        Variant {
            name: "E",
            glyph: Glyph::Cpu,
            text: "100%".into(),
            sizes: vec![(1, 16)],
            argb: WHITE,
        },
    ];
    async_io::block_on(async {
        let mut handles = Vec::new();
        for variant in variants {
            // Printed so the results document records the sizes actually offered, not ones
            // worked out by hand.
            for &(dot, height) in &variant.sizes {
                let icon = render(variant.glyph, &variant.text, dot, height, variant.argb);
                println!(
                    "variant {}: offers {}x{}",
                    variant.name, icon.width, icon.height
                );
            }
            handles.push(bounded("tray item spawn", SETUP_TIMEOUT, variant.spawn()).await??);
        }
        println!(
            "pid {}: five items shown; quit from any item's menu",
            std::process::id()
        );
        let cycle = ["7%", "37%", "100%"];
        let mut ticks = async_io::Timer::interval(Duration::from_secs(2));
        let mut step = 0;
        loop {
            ticks.next().await;
            step = (step + 1) % cycle.len();
            bounded(
                "variant A update",
                SETUP_TIMEOUT,
                handles[0].update(|variant: &mut Variant| variant.text = cycle[step].to_string()),
            )
            .await?
            .ok_or("variant A tray service shut down")?;
        }
    })
}

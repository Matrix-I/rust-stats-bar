//! Spike S3: does offering a larger pixmap keep the 20×20 labels sharp at 125% display scale?
//!
//! At 125% the AppIndicator extension draws each item in a 16 logical (20 device) pixel box at a
//! fractional device position, so every 1-pixel stroke of a 20×20 pixmap blends over two device
//! columns. The extension picks the smallest pixmap at least `16 × ceil(scale)` = 32 pixels wide,
//! so a 40×40 or 80×80 copy is sampled at a finer offset. Shows six items side by side; compare
//! them in a screenshot and record what you see in docs/spikes/.

use futures_lite::future;
use ksni::TrayMethods;
use statsbar_core::label::{self, Metric};
use statsbar_tray_spike::{SETUP_TIMEOUT, bounded};

const SIZE: usize = label::SIZE;

struct Variant {
    /// Single letter in the item's title, so a screenshot can name it.
    name: &'static str,
    icon: ksni::Icon,
}

impl ksni::Tray for Variant {
    fn id(&self) -> String {
        format!("statsbar-spike-sharp-{}", self.name)
    }
    fn title(&self) -> String {
        format!("Variant {}", self.name)
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
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

/// Copies each source pixel into a `factor`×`factor` block, then also inks `bold` extra texels to
/// the right of and below every inked block, which thickens strokes by `bold / factor` of a device
/// pixel without touching the 20×20 design.
fn scaled(source: &[u8], factor: usize, bold: usize) -> ksni::Icon {
    let side = SIZE * factor;
    let mut data = vec![0u8; side * side * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let at = (y * SIZE + x) * 4;
            if source[at] == 0 {
                continue;
            }
            for dy in 0..factor + bold {
                for dx in 0..factor + bold {
                    let (tx, ty) = (x * factor + dx, y * factor + dy);
                    if tx < side && ty < side {
                        let to = (ty * side + tx) * 4;
                        data[to..to + 4].copy_from_slice(&source[at..at + 4]);
                    }
                }
            }
        }
    }
    let side = i32::try_from(side).expect("at most 80 pixels");
    ksni::Icon {
        width: side,
        height: side,
        data,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cpu = label::render(Metric::Cpu, 37).ok_or("37 is in range")?;
    let rates = label::render_rates(12_000.0, 1_500_000.0);
    let variants = [
        // A: the shipped pixmap, the control.
        ("A", scaled(&cpu, 1, 0)),
        // B: the same drawing doubled.
        ("B", scaled(&cpu, 2, 0)),
        // C: the same drawing quadrupled.
        ("C", scaled(&cpu, 4, 0)),
        // D: quadrupled with strokes one texel (a quarter device pixel) wider.
        ("D", scaled(&cpu, 4, 1)),
        // E: the network label as shipped.
        ("E", scaled(&rates, 1, 0)),
        // F: the network label quadrupled.
        ("F", scaled(&rates, 4, 0)),
    ];
    async_io::block_on(async {
        let mut handles = Vec::new();
        for (name, icon) in variants {
            // Printed so the results document records the size actually offered.
            println!("variant {name}: offers {}x{}", icon.width, icon.height);
            let item = Variant { name, icon };
            handles.push(bounded("tray item spawn", SETUP_TIMEOUT, item.spawn()).await??);
        }
        println!(
            "pid {}: six items shown; quit from any item's menu",
            std::process::id()
        );
        future::pending::<()>().await;
        Ok(())
    })
}

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings, render};

/// PDF page (0-based) -> PNG bytes; scale 2.0 ≈ 144 dpi
pub fn pdf_to_png(pdf: Vec<u8>, page: usize, scale: f32) -> Option<Vec<u8>> {
    let pdf = Pdf::new(pdf).ok()?;
    let page = pdf.pages().get(page)?;
    let pixmap = render(
        page,
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &PixmapSettings {
            x_scale: scale,
            y_scale: scale,
            bg_color: WHITE,
        },
    );
    pixmap.into_png().ok()
}

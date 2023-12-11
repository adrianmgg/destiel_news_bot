use std::{path::PathBuf, io::Write};
use miette::{Result, IntoDiagnostic, Context};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ImageGenConfig {
    pub headline_bounds: Rect,
    pub max_font_size: i32,
    pub template: PathBuf,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn generate_image<W: Write>(config: &ImageGenConfig, text: &str, out: &mut W) -> Result<()> {
    let mut infile = std::fs::File::open(&config.template)
        .into_diagnostic()
        .wrap_err("failed to open template image file")?;
    let img = cairo::ImageSurface::create_from_png(&mut infile)
        .into_diagnostic()
        .wrap_err("failed to load template image")?;
    drop(infile);

    let pango_scale = pango::units_from_double(1.0);

    let ctx = cairo::Context::new(img).into_diagnostic()?;

    let layout = pangocairo::create_layout(&ctx);
    layout.set_text(text);
    layout.set_alignment(pango::Alignment::Center);
    layout.set_wrap(pango::WrapMode::WordChar);
    layout.set_width(config.headline_bounds.width * pango_scale);
    layout.set_height(config.headline_bounds.height * pango_scale);

    let mut desc = pango::FontDescription::new();
    desc.set_family("Impact");
    // desc.set_stretch(pango::Stretch::Condensed);
    desc.set_size(config.max_font_size * pango_scale);
    layout.set_font_description(Some(&desc));

    // shrink font as needed if the text doesn't fit
    loop {
        let (_, logical_pixel_bounds) = layout.pixel_extents();
        if logical_pixel_bounds.height() > config.headline_bounds.height {
            desc.set_size(desc.size() - pango_scale);
            layout.set_font_description(Some(&desc));
        } else {
            break;
        }
    }

    ctx.move_to(config.headline_bounds.x.into(), config.headline_bounds.y.into());
    ctx.set_source_rgb(0.0, 0.0, 0.0);
    pangocairo::layout_path(&ctx, &layout);
    ctx.set_line_width(4.0);
    ctx.stroke().into_diagnostic()?;

    ctx.move_to(config.headline_bounds.x.into(), config.headline_bounds.y.into());
    ctx.set_source_rgb(1.0, 1.0, 1.0);
    pangocairo::layout_path(&ctx, &layout);
    ctx.fill().into_diagnostic()?;

    ctx.target().write_to_png(out).into_diagnostic()?;
    Ok(())
}

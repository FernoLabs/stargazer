use crate::{context::SharedState, models::ChainEvent, protocols};
use ab_glyph::{Font, FontArc, GlyphId, OutlinedGlyph, PxScale, Rect, ScaleFont, point};
use anyhow::{Context, Result};
use image::{
    DynamicImage, ImageFormat, Rgba, RgbaImage,
    imageops::{self, FilterType},
};
use reqwest::Client;
use serde_json::Value;
use std::{io::Cursor, sync::OnceLock, time::Duration};
use tokio::time::timeout;

const STANDARD_TEMPLATE: &[u8] =
    include_bytes!("../../frontend/static/brand/firewatch-preview-standard.png");
const HIGH_TEMPLATE: &[u8] =
    include_bytes!("../../frontend/static/brand/firewatch-preview-high.png");
const CRITICAL_TEMPLATE: &[u8] =
    include_bytes!("../../frontend/static/brand/firewatch-preview-critical.png");
const FIREWATCH_MARK: &[u8] =
    include_bytes!("../../frontend/static/brand/firewatch-hero-logo-01-clean.png");
const LOGO_FETCH_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_LOGO_BYTES: usize = 2_000_000;

const FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
];

pub async fn render_event_preview_card(state: &SharedState, event: &ChainEvent) -> Result<Vec<u8>> {
    let data = PreviewCardData::from_event(event);
    let mut canvas = data.level.decode_template()?;
    let font = preview_font()?;

    draw_card_base(&mut canvas, &data, font);
    draw_event_panel(&mut canvas, &data, font);

    if let Some(protocol_name) = data.protocol_name.as_deref() {
        match fetch_protocol_logo(&state.http_client, data.protocol_slug.as_deref()).await {
            Some(logo) => draw_protocol_logo(&mut canvas, &logo),
            None => draw_protocol_monogram(&mut canvas, font, &data, protocol_name),
        }
    }

    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut cursor, ImageFormat::Png)
        .context("encode preview card png")?;

    Ok(cursor.into_inner())
}

#[derive(Clone, Copy)]
enum AlertLevel {
    Critical,
    High,
    Standard,
}

impl AlertLevel {
    fn from_event(event: &ChainEvent) -> Self {
        if event.is_critical_signal() {
            return Self::Critical;
        }

        if event
            .tags
            .iter()
            .any(|tag| matches!(tag.as_str(), "severity:high" | "impact:high"))
        {
            return Self::High;
        }

        Self::Standard
    }

    fn label(self) -> &'static str {
        match self {
            Self::Critical => "Critical alert",
            Self::High => "High alert",
            Self::Standard => "Standard alert",
        }
    }

    fn accent(self) -> Rgba<u8> {
        match self {
            Self::Critical => Rgba([239, 101, 58, 255]),
            Self::High => Rgba([238, 169, 79, 255]),
            Self::Standard => Rgba([112, 214, 173, 255]),
        }
    }

    fn decode_template(self) -> Result<RgbaImage> {
        let bytes = match self {
            Self::Critical => CRITICAL_TEMPLATE,
            Self::High => HIGH_TEMPLATE,
            Self::Standard => STANDARD_TEMPLATE,
        };

        Ok(image::load_from_memory(bytes)
            .context("decode preview card template")?
            .into_rgba8())
    }
}

struct PreviewCardData {
    level: AlertLevel,
    protocol_slug: Option<String>,
    protocol_name: Option<String>,
    title: String,
    summary: String,
    event_type: String,
    target: String,
    slot: String,
}

impl PreviewCardData {
    fn from_event(event: &ChainEvent) -> Self {
        let protocol_slug = event_protocol_slug(event);
        let protocol_name = payload_string(event, "protocol_name")
            .map(ToOwned::to_owned)
            .or_else(|| {
                protocol_slug
                    .as_deref()
                    .and_then(protocols::curated_protocol_descriptor)
                    .map(|protocol| protocol.name)
            });
        let descriptors = protocols::curated_protocol_descriptors();

        Self {
            level: AlertLevel::from_event(event),
            protocol_slug,
            protocol_name: protocol_name.clone(),
            title: event_title(event, protocol_name.as_deref(), &descriptors),
            summary: compact_long_tokens(&event.summary),
            event_type: event_kind_label(event).to_owned(),
            target: event_target_display_name(event, &descriptors),
            slot: format_number(event.slot),
        }
    }
}

async fn fetch_protocol_logo(client: &Client, slug: Option<&str>) -> Option<RgbaImage> {
    let logo_id = protocol_logo_id(slug?)?;
    let url =
        format!("https://raw.githubusercontent.com/jup-ag/platform-list/main/img/{logo_id}.webp");

    let response = timeout(LOGO_FETCH_TIMEOUT, client.get(url).send())
        .await
        .ok()?
        .ok()?;
    if !response.status().is_success() {
        return None;
    }

    let bytes = timeout(LOGO_FETCH_TIMEOUT, response.bytes())
        .await
        .ok()?
        .ok()?;
    if bytes.len() > MAX_LOGO_BYTES {
        return None;
    }

    image::load_from_memory(&bytes)
        .ok()
        .map(DynamicImage::into_rgba8)
}

fn draw_event_panel(canvas: &mut RgbaImage, data: &PreviewCardData, font: &FontArc) {
    blend_rect(canvas, 318, 270, 790, 258, Rgba([14, 18, 23, 242]));

    let title_lines = wrap_text(font, &data.title, 38.0, 720.0, 2);
    for (index, line) in title_lines.iter().enumerate() {
        draw_text_mut(
            canvas,
            Rgba([248, 248, 246, 255]),
            336,
            292 + (index as i32 * 44),
            38.0,
            font,
            line,
        );
    }

    let summary_y = 292 + (title_lines.len() as i32 * 44) + 14;
    let summary = dedup_summary(&data.title, &data.summary);
    let summary_lines = wrap_text(font, &summary, 22.0, 712.0, 2);
    for (index, line) in summary_lines.iter().enumerate() {
        draw_text_mut(
            canvas,
            Rgba([181, 190, 201, 255]),
            336,
            summary_y + (index as i32 * 29),
            22.0,
            font,
            line,
        );
    }

    draw_pills(canvas, font, &preview_pills(data));
}

fn draw_card_base(canvas: &mut RgbaImage, data: &PreviewCardData, font: &FontArc) {
    fill_rect(
        canvas,
        0,
        0,
        canvas.width(),
        canvas.height(),
        Rgba([14, 18, 23, 255]),
    );

    draw_brand_mark(canvas, font);
    draw_text_mut(
        canvas,
        Rgba([248, 248, 246, 255]),
        232,
        82,
        58.0,
        font,
        "FireWatch",
    );
    draw_text_mut(
        canvas,
        data.level.accent(),
        234,
        150,
        28.0,
        font,
        data.level.label(),
    );
    draw_text_mut(
        canvas,
        Rgba([175, 184, 194, 255]),
        234,
        190,
        26.0,
        font,
        "Solana program and governance monitoring",
    );
    draw_text_mut(
        canvas,
        Rgba([235, 238, 242, 255]),
        72,
        508,
        27.0,
        font,
        "watch.ferno.ag",
    );
}

fn draw_brand_mark(canvas: &mut RgbaImage, font: &FontArc) {
    match image::load_from_memory(FIREWATCH_MARK).map(DynamicImage::into_rgba8) {
        Ok(mark) => {
            draw_centered_image(canvas, &mark, 72, 78, 132, 132);
        }
        Err(_) => {
            let center = (138, 144);
            draw_filled_circle_mut(canvas, center, 66, Rgba([248, 248, 246, 245]));
            draw_filled_circle_mut(canvas, center, 46, Rgba([23, 29, 35, 245]));
            let scale = 32.0;
            let width = text_width(font, scale, "FW").ceil() as i32;
            draw_text_mut(
                canvas,
                Rgba([238, 169, 79, 255]),
                center.0 - width / 2,
                center.1 - 20,
                scale,
                font,
                "FW",
            );
        }
    }
}

fn draw_pills(canvas: &mut RgbaImage, font: &FontArc, values: &[String]) {
    let mut x = 336;
    let y = 508;

    for value in values {
        let label = truncate_to_width(font, value, 18.0, 184.0);
        let width = (text_width(font, 18.0, &label).ceil() as u32 + 26).clamp(70, 214);
        blend_rect(canvas, x, y, width, 40, Rgba([34, 41, 49, 230]));
        draw_text_mut(
            canvas,
            Rgba([230, 235, 241, 255]),
            x + 14,
            y + 10,
            18.0,
            font,
            &label,
        );
        x += width as i32 + 10;
        if x > 1030 {
            break;
        }
    }
}

fn draw_protocol_logo(canvas: &mut RgbaImage, logo: &RgbaImage) {
    let x = 966;
    let y = 96;
    let size = 118;
    let center = (x + size / 2, y + size / 2);

    draw_filled_circle_mut(canvas, center, size / 2, Rgba([248, 248, 246, 245]));
    let logo = trim_image_to_content(logo);
    let fitted = resize_contain(&logo, (size - 32) as u32, (size - 32) as u32);
    overlay_centered_clipped(canvas, &fitted, center, (size / 2) - 8);
}

fn draw_protocol_monogram(
    canvas: &mut RgbaImage,
    font: &FontArc,
    data: &PreviewCardData,
    protocol_name: &str,
) {
    let x = 966;
    let y = 96;
    let size = 118;
    let center = (x + size / 2, y + size / 2);
    let monogram = protocol_monogram(data.protocol_slug.as_deref(), protocol_name);
    let scale = 40.0;
    let width = text_width(font, scale, &monogram).ceil() as i32;

    draw_filled_circle_mut(canvas, center, size / 2, Rgba([248, 248, 246, 245]));
    draw_text_mut(
        canvas,
        data.level.accent(),
        center.0 - width / 2,
        center.1 - 25,
        scale,
        font,
        &monogram,
    );
}

fn resize_contain(image: &RgbaImage, max_width: u32, max_height: u32) -> RgbaImage {
    let width = image.width().max(1);
    let height = image.height().max(1);
    let scale = (max_width as f32 / width as f32).min(max_height as f32 / height as f32);
    let next_width = ((width as f32 * scale).round() as u32).max(1);
    let next_height = ((height as f32 * scale).round() as u32).max(1);

    imageops::resize(image, next_width, next_height, FilterType::Lanczos3)
}

fn draw_centered_image(
    canvas: &mut RgbaImage,
    image: &RgbaImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    fill_rect(canvas, x, y, width, height, Rgba([248, 248, 246, 255]));

    let content = trim_image_to_content(image);
    let fitted = resize_contain(
        &content,
        width.saturating_sub(18),
        height.saturating_sub(18),
    );
    let overlay_x = x + (width as i32 - fitted.width() as i32) / 2;
    let overlay_y = y + (height as i32 - fitted.height() as i32) / 2;
    overlay_image(canvas, &fitted, overlay_x, overlay_y);
}

fn overlay_centered_clipped(
    canvas: &mut RgbaImage,
    image: &RgbaImage,
    center: (i32, i32),
    clip_radius: i32,
) {
    let x = center.0 - image.width() as i32 / 2;
    let y = center.1 - image.height() as i32 / 2;
    let radius_squared = clip_radius as i64 * clip_radius as i64;

    for source_y in 0..image.height() {
        for source_x in 0..image.width() {
            let target_x = x + source_x as i32;
            let target_y = y + source_y as i32;

            if target_x < 0
                || target_y < 0
                || target_x >= canvas.width() as i32
                || target_y >= canvas.height() as i32
            {
                continue;
            }

            let dx = (target_x - center.0) as i64;
            let dy = (target_y - center.1) as i64;
            if dx * dx + dy * dy > radius_squared {
                continue;
            }

            let source = image.get_pixel(source_x, source_y);
            let target = canvas.get_pixel_mut(target_x as u32, target_y as u32);
            *target = alpha_blend(*target, *source);
        }
    }
}

fn overlay_image(canvas: &mut RgbaImage, image: &RgbaImage, x: i32, y: i32) {
    for source_y in 0..image.height() {
        for source_x in 0..image.width() {
            let target_x = x + source_x as i32;
            let target_y = y + source_y as i32;

            if target_x < 0
                || target_y < 0
                || target_x >= canvas.width() as i32
                || target_y >= canvas.height() as i32
            {
                continue;
            }

            let source = image.get_pixel(source_x, source_y);
            let target = canvas.get_pixel_mut(target_x as u32, target_y as u32);
            *target = alpha_blend(*target, *source);
        }
    }
}

fn trim_image_to_content(image: &RgbaImage) -> RgbaImage {
    let has_transparency = image.pixels().any(|pixel| pixel[3] < 245);
    let mut left = image.width();
    let mut top = image.height();
    let mut right = 0;
    let mut bottom = 0;

    for (x, y, pixel) in image.enumerate_pixels() {
        let visible = if has_transparency {
            pixel[3] > 16
        } else {
            !(pixel[0] > 244 && pixel[1] > 244 && pixel[2] > 244)
        };

        if visible {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }

    if left > right || top > bottom {
        return image.clone();
    }

    imageops::crop_imm(image, left, top, right - left + 1, bottom - top + 1).to_image()
}

fn fill_rect(canvas: &mut RgbaImage, x: i32, y: i32, width: u32, height: u32, color: Rgba<u8>) {
    let left = x.max(0) as u32;
    let top = y.max(0) as u32;
    let right = (x + width as i32).min(canvas.width() as i32).max(0) as u32;
    let bottom = (y + height as i32).min(canvas.height() as i32).max(0) as u32;

    for pixel_y in top..bottom {
        for pixel_x in left..right {
            *canvas.get_pixel_mut(pixel_x, pixel_y) = color;
        }
    }
}

fn blend_rect(canvas: &mut RgbaImage, x: i32, y: i32, width: u32, height: u32, color: Rgba<u8>) {
    let left = x.max(0) as u32;
    let top = y.max(0) as u32;
    let right = (x + width as i32).min(canvas.width() as i32).max(0) as u32;
    let bottom = (y + height as i32).min(canvas.height() as i32).max(0) as u32;

    for pixel_y in top..bottom {
        for pixel_x in left..right {
            let base = canvas.get_pixel_mut(pixel_x, pixel_y);
            *base = alpha_blend(*base, color);
        }
    }
}

fn draw_filled_circle_mut(
    canvas: &mut RgbaImage,
    center: (i32, i32),
    radius: i32,
    color: Rgba<u8>,
) {
    let radius_squared = radius as i64 * radius as i64;
    let left = (center.0 - radius).max(0);
    let right = (center.0 + radius).min(canvas.width() as i32 - 1);
    let top = (center.1 - radius).max(0);
    let bottom = (center.1 + radius).min(canvas.height() as i32 - 1);

    for y in top..=bottom {
        for x in left..=right {
            let dx = (x - center.0) as i64;
            let dy = (y - center.1) as i64;
            if dx * dx + dy * dy <= radius_squared {
                let base = canvas.get_pixel_mut(x as u32, y as u32);
                *base = alpha_blend(*base, color);
            }
        }
    }
}

fn draw_text_mut(
    canvas: &mut RgbaImage,
    color: Rgba<u8>,
    x: i32,
    y: i32,
    font_size: f32,
    font: &FontArc,
    text: &str,
) {
    let image_width = canvas.width() as i32;
    let image_height = canvas.height() as i32;

    layout_glyphs(font, font_size, text, |glyph, bounds| {
        glyph.draw(|glyph_x, glyph_y, coverage| {
            let image_x = glyph_x as i32 + x + bounds.min.x.round() as i32;
            let image_y = glyph_y as i32 + y + bounds.min.y.round() as i32;

            if (0..image_width).contains(&image_x) && (0..image_height).contains(&image_y) {
                let pixel = canvas.get_pixel_mut(image_x as u32, image_y as u32);
                *pixel = alpha_blend_coverage(*pixel, color, coverage);
            }
        });
    });
}

fn layout_glyphs(
    font: &FontArc,
    font_size: f32,
    text: &str,
    mut draw: impl FnMut(OutlinedGlyph, Rect),
) {
    let scale = PxScale::from(font_size);
    let scaled_font = font.as_scaled(scale);
    let mut cursor_x = 0.0;
    let mut previous: Option<GlyphId> = None;

    for character in text.chars() {
        let glyph_id = scaled_font.glyph_id(character);
        if let Some(previous) = previous {
            cursor_x += scaled_font.kern(previous, glyph_id);
        }

        let glyph = glyph_id.with_scale_and_position(scale, point(cursor_x, scaled_font.ascent()));
        cursor_x += scaled_font.h_advance(glyph_id);
        previous = Some(glyph_id);

        if let Some(outlined) = scaled_font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            draw(outlined, bounds);
        }
    }
}

fn alpha_blend(base: Rgba<u8>, overlay: Rgba<u8>) -> Rgba<u8> {
    let alpha = overlay[3] as u16;
    let inverse = 255 - alpha;

    Rgba([
        ((overlay[0] as u16 * alpha + base[0] as u16 * inverse) / 255) as u8,
        ((overlay[1] as u16 * alpha + base[1] as u16 * inverse) / 255) as u8,
        ((overlay[2] as u16 * alpha + base[2] as u16 * inverse) / 255) as u8,
        255,
    ])
}

fn alpha_blend_coverage(base: Rgba<u8>, overlay: Rgba<u8>, coverage: f32) -> Rgba<u8> {
    let alpha = (overlay[3] as f32 * coverage.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;

    alpha_blend(base, Rgba([overlay[0], overlay[1], overlay[2], alpha]))
}

fn preview_font() -> Result<&'static FontArc> {
    static PREVIEW_FONT: OnceLock<Option<FontArc>> = OnceLock::new();

    PREVIEW_FONT
        .get_or_init(|| {
            FONT_PATHS.iter().find_map(|path| {
                let bytes = std::fs::read(path).ok()?;
                FontArc::try_from_vec(bytes).ok()
            })
        })
        .as_ref()
        .context("no preview font found; install DejaVu Sans or Arial")
}

fn wrap_text(
    font: &FontArc,
    text: &str,
    font_size: f32,
    max_width: f32,
    max_lines: usize,
) -> Vec<String> {
    if text.trim().is_empty() || max_lines == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    let words = text.split_whitespace().collect::<Vec<_>>();

    for word in words {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };

        if text_width(font, font_size, &candidate) <= max_width {
            current = candidate;
            continue;
        }

        if !current.is_empty() {
            lines.push(current);
        } else {
            lines.push(truncate_to_width(font, word, font_size, max_width));
        }

        if lines.len() == max_lines {
            let last = lines.pop().unwrap_or_default();
            lines.push(truncate_to_width(
                font,
                &format!("{last}..."),
                font_size,
                max_width,
            ));
            return lines;
        }

        current = word.to_owned();
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }

    if lines.len() == max_lines {
        let remaining_width = text_width(font, font_size, lines.last().unwrap_or(&String::new()));
        if remaining_width > max_width {
            let last = lines.pop().unwrap_or_default();
            lines.push(truncate_to_width(font, &last, font_size, max_width));
        }
    }

    lines
}

fn truncate_to_width(font: &FontArc, text: &str, font_size: f32, max_width: f32) -> String {
    if text_width(font, font_size, text) <= max_width {
        return text.to_owned();
    }

    let mut truncated = text.to_owned();
    while !truncated.is_empty() {
        truncated.pop();
        let candidate = format!("{}...", truncated.trim_end());
        if text_width(font, font_size, &candidate) <= max_width {
            return candidate;
        }
    }

    "...".to_owned()
}

fn text_width(font: &FontArc, font_size: f32, text: &str) -> f32 {
    let scaled = font.as_scaled(PxScale::from(font_size));
    let mut width = 0.0;
    let mut previous = None;

    for character in text.chars() {
        let glyph_id = scaled.glyph_id(character);
        if let Some(previous) = previous {
            width += scaled.kern(previous, glyph_id);
        }
        width += scaled.h_advance(glyph_id);
        previous = Some(glyph_id);
    }

    width
}

fn event_protocol_slug(event: &ChainEvent) -> Option<String> {
    event.tags.iter().find_map(|tag| {
        tag.strip_prefix("protocol:")
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn payload_string<'a>(event: &'a ChainEvent, key: &str) -> Option<&'a str> {
    match event.payload.get(key) {
        Some(Value::String(value)) if !value.is_empty() => Some(value),
        _ => None,
    }
}

fn event_target_display_name(
    event: &ChainEvent,
    descriptors: &[crate::models::DefiProtocolDescriptor],
) -> String {
    if let Some(domain_name) =
        payload_string(event, "domain_name").or_else(|| payload_string(event, "target_name"))
    {
        return domain_name.to_owned();
    }

    for protocol in descriptors {
        for program in &protocol.programs {
            if program.program_id == event.target_address {
                let protocol_name = &protocol.name;
                let program_label = &program.label;
                let protocol_lower = protocol_name.to_ascii_lowercase();
                let program_lower = program_label.to_ascii_lowercase();

                if protocol_lower.contains(&program_lower) || program_lower == "core" {
                    return protocol_name.clone();
                }

                return format!("{protocol_name} {program_label}");
            }
        }
    }

    payload_string(event, "program_label")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| short_address(&event.target_address))
}

fn event_title(
    event: &ChainEvent,
    protocol_name: Option<&str>,
    descriptors: &[crate::models::DefiProtocolDescriptor],
) -> String {
    let instruction = payload_string(event, "instruction_name");
    let target_name = event_target_display_name(event, descriptors);

    match event.kind.as_str() {
        "name_service_domain_created" => format!("{target_name} created"),
        "name_service_domain_updated" => format!("{target_name} updated"),
        "name_service_domain_transferred" => format!("{target_name} ownership changed"),
        "name_service_domain_deleted" => format!("{target_name} deleted"),
        "name_service_domain_reallocated" => format!("{target_name} resized"),
        "program_unknown_instruction" => match protocol_name {
            Some(protocol) => format!("{protocol} unknown instruction"),
            None => "Unknown program instruction".to_owned(),
        },
        "program_upgraded" => format!("{target_name} upgraded"),
        "program_deployed" => format!("{target_name} deployed"),
        "program_finalized" => format!("{target_name} finalized"),
        "program_migrated_to_loader_v4" => format!("{target_name} migrated to loader v4"),
        "program_admin_instruction" => match (protocol_name, instruction) {
            (Some(protocol), Some(instruction)) => {
                format!("{protocol} {}", readable_instruction_name(instruction))
            }
            _ => "Admin instruction observed".to_owned(),
        },
        "program_metric_alert" => match protocol_name {
            Some(protocol) => format!("{protocol} metric anomaly"),
            None => "Metric anomaly observed".to_owned(),
        },
        "program_idl_updated" => protocol_event_title(protocol_name, "IDL updated"),
        "program_idl_initialized" => protocol_event_title(protocol_name, "IDL initialized"),
        "program_idl_authority_changed" => {
            protocol_event_title(protocol_name, "IDL authority changed")
        }
        "program_idl_immutable" => protocol_event_title(protocol_name, "IDL immutable"),
        "program_idl_resized" => protocol_event_title(protocol_name, "IDL resized"),
        "program_idl_closed" => protocol_event_title(protocol_name, "IDL closed"),
        "multisig_proposal_activated" => "Squads proposal activated".to_owned(),
        "multisig_proposal_approved" => "Squads proposal approval".to_owned(),
        "multisig_proposal_rejected" => "Squads proposal rejected".to_owned(),
        "multisig_proposal_cancelled" => "Squads proposal cancelled".to_owned(),
        "multisig_vault_executed" => "Squads vault execution".to_owned(),
        "multisig_config_executed" => "Squads config execution".to_owned(),
        _ => compact_long_tokens(&event.summary),
    }
}

fn preview_pills(data: &PreviewCardData) -> Vec<String> {
    let mut values = vec![data.event_type.clone(), format!("Slot {}", data.slot)];
    if !contains_case_insensitive(&data.title, &data.target) {
        values.insert(1, data.target.clone());
    }
    values
}

fn dedup_summary(title: &str, summary: &str) -> String {
    let normalized_title = normalize_preview_text(title);
    let normalized_summary = normalize_preview_text(summary);
    if normalized_summary.is_empty()
        || normalized_summary == normalized_title
        || normalized_summary.contains(&normalized_title)
    {
        return String::new();
    }

    let lower_summary = summary.to_ascii_lowercase();
    if let Some(index) = lower_summary.find(" via ") {
        return capitalize_first(summary[index + 1..].trim());
    }

    if let Some(stripped) = summary.strip_prefix("High-impact ") {
        return stripped.to_owned();
    }

    summary.to_owned()
}

fn capitalize_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
    if needle.trim().is_empty() {
        return false;
    }
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn normalize_preview_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_ascii_lowercase()
}

fn protocol_event_title(protocol_name: Option<&str>, label: &str) -> String {
    match protocol_name {
        Some(protocol) => format!("{protocol} {label}"),
        None => label.to_owned(),
    }
}

fn event_kind_label(event: &ChainEvent) -> &'static str {
    match event.kind.as_str() {
        "name_service_domain_created" => "Domain create",
        "name_service_domain_updated" => "Domain update",
        "name_service_domain_transferred" => "Domain transfer",
        "name_service_domain_deleted" => "Domain delete",
        "name_service_domain_reallocated" => "Domain resize",
        "program_unknown_instruction" => "Unknown instruction",
        "program_admin_instruction" => "Admin instruction",
        "program_upgraded" => "Program upgrade",
        "program_deployed" => "Program deploy",
        "program_finalized" => "Program finalized",
        "program_migrated_to_loader_v4" => "Loader migration",
        "program_metric_alert" => "Metric anomaly",
        "program_idl_initialized"
        | "program_idl_updated"
        | "program_idl_authority_changed"
        | "program_idl_immutable"
        | "program_idl_resized"
        | "program_idl_closed" => "IDL change",
        "multisig_proposal_activated" => "Squads proposal",
        "multisig_proposal_approved" => "Squads approval",
        "multisig_proposal_rejected" => "Squads rejection",
        "multisig_proposal_cancelled" => "Squads cancellation",
        "multisig_vault_executed" => "Squads execution",
        "multisig_config_executed" => "Squads config",
        _ => "Chain event",
    }
}

fn readable_instruction_name(instruction: &str) -> String {
    instruction
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn compact_long_tokens(input: &str) -> String {
    input
        .split_whitespace()
        .map(|token| {
            if token.len() >= 32
                && token
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
            {
                format!("{}...{}", &token[..6], &token[token.len() - 6..])
            } else {
                token.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn short_address(address: &str) -> String {
    if address.len() < 12 {
        return address.to_owned();
    }

    format!("{}...{}", &address[..4], &address[address.len() - 4..])
}

fn format_number(value: i64) -> String {
    let raw = value.abs().to_string();
    let mut formatted = String::new();

    for (index, character) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(character);
    }

    let mut formatted = formatted.chars().rev().collect::<String>();
    if value < 0 {
        formatted.insert(0, '-');
    }

    formatted
}

fn protocol_logo_id(slug: &str) -> Option<String> {
    if slug == "ferno" {
        return None;
    }

    let explicit = match slug {
        "orca_whirlpools" => "orca",
        "kamino_lend" => "kamino",
        "drift" => "drift",
        "marginfi" => "marginfi",
        "jupiter_lend" => "jupiter-exchange",
        "raydium" => "raydium",
        "meteora_dlmm" | "meteora" => "meteora",
        "exponent" => "exponent",
        "carrot" => "carrot",
        "save" => "save",
        "openbook" => "openbook",
        "manifest" => "manifest",
        "glam" => "glam",
        "adrena" => "adrena",
        "loopscale" => "loopscale",
        "neutral" => "neutral",
        "fragmetric" => "fragmetric",
        "marinade" => "marinade",
        "jito" => "jito",
        "kamino" => "kamino",
        "parcl" => "parcl",
        "perena" => "perena",
        "sanctum" => "sanctum",
        "solayer" => "solayer",
        "huma" => "huma",
        "maple" => "maple",
        "flashtrade" => "flash",
        "wasabi" => "wasabi",
        _ => return Some(slug.replace('_', "-")),
    };

    Some(explicit.to_owned())
}

fn protocol_monogram(slug: Option<&str>, label: &str) -> String {
    match slug {
        Some("meteora_dlmm") => return "DL".to_owned(),
        Some("jupiter_lend") => return "JL".to_owned(),
        Some("orca_whirlpools") => return "OR".to_owned(),
        Some("raydium") => return "RY".to_owned(),
        Some("kamino_lend") => return "KA".to_owned(),
        Some("drift") => return "DR".to_owned(),
        Some("marginfi") => return "MF".to_owned(),
        _ => {}
    }

    let monogram = label
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_ascii_uppercase();

    if monogram.is_empty() {
        "FW".to_owned()
    } else {
        monogram
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_preview_card_layers() {
        let event = ChainEvent {
            id: "event-1".to_owned(),
            signature: "signature".to_owned(),
            instruction_path: "0".to_owned(),
            kind: "program_admin_instruction".to_owned(),
            target_type: "program".to_owned(),
            target_address: "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo".to_owned(),
            actor: None,
            slot: 123_456_789,
            block_time: None,
            indexed_at: 0,
            summary: "Meteora DLMM admin instruction withdraw_protocol_fee observed".to_owned(),
            tags: vec![
                "protocol:meteora_dlmm".to_owned(),
                "severity:high".to_owned(),
            ],
            payload: json!({
                "protocol_name": "Meteora DLMM",
                "instruction_name": "withdraw_protocol_fee"
            }),
            delivery_attempts: 0,
            delivered_at: None,
            last_error: None,
            next_retry_at: 0,
        };
        let data = PreviewCardData::from_event(&event);
        let Ok(font) = preview_font() else {
            return;
        };
        let mut canvas = data.level.decode_template().expect("template decodes");

        draw_card_base(&mut canvas, &data, font);
        draw_event_panel(&mut canvas, &data, font);
        draw_protocol_monogram(&mut canvas, font, &data, "Meteora DLMM");

        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(canvas)
            .write_to(&mut cursor, ImageFormat::Png)
            .expect("preview card encodes");

        assert!(cursor.into_inner().len() > 10_000);
    }
}

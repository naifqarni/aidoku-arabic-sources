//! Draws the reader's text overlays (translated speech bubbles) onto a page image.
use aidoku::{
	alloc::{String, Vec, collections::BTreeMap},
	imports::canvas::{Canvas, Color, Font, FontWeight, ImageRef, Point, Rect},
	prelude::*,
};

use crate::{BASE_URL, models::Bubble};

const DEFAULT_FAMILY: &str = "Hacen Samra";
const FALLBACK_FAMILY: &str = "tajawal";
const DEFAULT_LINE_HEIGHT: f32 = 1.2;
const LINE_TOLERANCE: f32 = 1.15;
const MAX_FIT_PASSES: usize = 2;

const FONT_PATHS: &[(&str, &str)] = &[
	("hayah", "/fonts/manga/66Hayah.otf"),
	("hacen samra", "/fonts/manga/Hacen%20Samra%20Lt.ttf"),
	("kharabeesh", "/fonts/manga/81kharabeesh.ttf"),
	("boahmed alhour", "/fonts/manga/Boahmed%20Alhour.ttf"),
	("mangathinking", "/fonts/manga/thinking.ttf?v=3"),
	("khat al taradod", "/fonts/manga/Khat_Al_Taradod.ttf"),
	("elmessiri", "/fonts/manga/ElMessiri-SemiBold.ttf"),
	("tajawal", "/fonts/manga/Tajawal-Medium.ttf"),
];

fn font_key(family: &str) -> String {
	family
		.split(',')
		.next()
		.unwrap_or_default()
		.trim()
		.trim_matches(['"', '\''])
		.trim()
		.to_lowercase()
}

fn font_path(family: &str) -> Option<&'static str> {
	let key = font_key(family);
	let find = |k: &str| {
		FONT_PATHS
			.iter()
			.find(|(name, _)| *name == k)
			.map(|(_, p)| *p)
	};
	find(&key).or_else(|| find(FALLBACK_FAMILY))
}

fn load_font(family: &str) -> Font {
	font_path(family)
		.and_then(|path| Font::load(&format!("{BASE_URL}{path}")).ok())
		.unwrap_or_else(|| Font::system(FontWeight::Bold))
}

/// The canvas API cannot measure text, so widths are estimated per glyph in ems.
fn glyph_width(c: char) -> f32 {
	match c {
		// Arabic harakat and other combining marks take no space
		'\u{064B}'..='\u{065F}'
		| '\u{0670}'
		| '\u{06D6}'..='\u{06ED}'
		| '\u{0300}'..='\u{036F}' => 0.0,
		' ' => 0.28,
		'.' | ',' | '!' | '؟' | '?' | '،' | ':' | ';' | '\'' | '"' | '|' | 'i' | 'l' => 0.3,
		'\u{0600}'..='\u{06FF}'
		| '\u{0750}'..='\u{077F}'
		| '\u{FB50}'..='\u{FDFF}'
		| '\u{FE70}'..='\u{FEFF}' => 0.5,
		c if c.is_ascii() => 0.55,
		_ => 1.0,
	}
}

fn measure(text: &str, size: f32) -> f32 {
	text.chars().map(glyph_width).sum::<f32>() * size
}

fn wrap_lines(text: &str, size: f32, max_width: f32) -> Vec<String> {
	let mut result = Vec::new();
	for line in text.split('\n') {
		if measure(line, size) <= max_width * LINE_TOLERANCE || !line.contains(' ') {
			result.push(line.into());
			continue;
		}
		let mut current = String::new();
		for word in line.split(' ') {
			let candidate = if current.is_empty() {
				String::from(word)
			} else {
				format!("{current} {word}")
			};
			if !current.is_empty() && measure(&candidate, size) > max_width {
				result.push(core::mem::replace(&mut current, word.into()));
			} else {
				current = candidate;
			}
		}
		if !current.is_empty() {
			result.push(current);
		}
	}
	result
}

fn parse_color(color: Option<&str>) -> Option<Color> {
	let color = color?.trim().to_lowercase();
	if let Some(hex) = color.strip_prefix('#') {
		let digit = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
		let short = |i: usize| u8::from_str_radix(hex.get(i..=i)?, 16).ok().map(|v| v * 17);
		let (r, g, b, a) = match hex.len() {
			3 => (short(0)?, short(1)?, short(2)?, 255),
			4 => (short(0)?, short(1)?, short(2)?, short(3)?),
			6 => (digit(0)?, digit(2)?, digit(4)?, 255),
			// CSS order: #rrggbbaa
			8 => (digit(0)?, digit(2)?, digit(4)?, digit(6)?),
			_ => return None,
		};
		return Some(Color::new(r as f32, g as f32, b as f32, a as f32 / 255.0));
	}
	if let Some(args) = color
		.strip_prefix("rgba(")
		.or_else(|| color.strip_prefix("rgb("))
		.and_then(|s| s.strip_suffix(')'))
	{
		let parts: Vec<f32> = args
			.split([',', ' ', '/'])
			.filter(|s| !s.is_empty())
			.filter_map(|s| s.trim().parse().ok())
			.collect();
		return match parts.as_slice() {
			[r, g, b] => Some(Color::new(*r, *g, *b, 1.0)),
			[r, g, b, a] => Some(Color::new(*r, *g, *b, *a)),
			_ => None,
		};
	}
	match color.as_str() {
		"black" => Some(Color::black()),
		"white" => Some(Color::white()),
		"red" => Some(Color::red()),
		_ => None,
	}
}

/// `source_size` is the page size the overlay coordinates were authored at, if known.
pub fn paint(image: &ImageRef, bubbles: &[Bubble], source_size: Option<(f32, f32)>) -> ImageRef {
	let width = image.width();
	let height = image.height();
	let scale = source_size.map_or((1.0, 1.0), |(w, h)| (width / w, height / h));
	let mut canvas = Canvas::new(width, height);
	canvas.draw_image(image, Rect::new(0.0, 0.0, width, height));

	let mut fonts: BTreeMap<String, Font> = BTreeMap::new();
	for bubble in bubbles.iter().filter(|b| !b.text.trim().is_empty()) {
		let family = if bubble.font_family.trim().is_empty() {
			DEFAULT_FAMILY
		} else {
			bubble.font_family.as_str()
		};
		let key = font_key(family);
		if !fonts.contains_key(&key) {
			fonts.insert(key.clone(), load_font(family));
		}
		if let Some(font) = fonts.get(&key) {
			draw_bubble(&mut canvas, bubble, font, (width, height), scale);
		}
	}
	canvas.get_image()
}

fn draw_bubble(
	canvas: &mut Canvas,
	bubble: &Bubble,
	font: &Font,
	(width, height): (f32, f32),
	(sx, sy): (f32, f32),
) {
	let (x, y, w, h) = (bubble.x * sx, bubble.y * sy, bubble.w * sx, bubble.h * sy);
	let font_scale = (sx + sy) / 2.0;
	let left = (width - w).min(x).max(0.0);
	let top = (height - h).min(y).max(0.0);
	let box_width = (width - left).min(w);
	let box_height = (height - top).min(h);
	if box_width <= 0.0 || box_height <= 0.0 {
		return;
	}

	let line_height = if bubble.line_height > 0.0 {
		bubble.line_height
	} else {
		DEFAULT_LINE_HEIGHT
	};
	let max_width = (box_width - 2.0).max(1.0);

	let mut size = (bubble.font_size_px * font_scale).max(1.0);
	let mut lines = wrap_lines(&bubble.text, size, max_width);
	for _ in 0..MAX_FIT_PASSES {
		let widest = lines
			.iter()
			.map(|l| measure(l, size))
			.fold(0.0f32, f32::max);
		let scale = 1.0f32
			.min(max_width / widest.max(1.0))
			.min(box_height / (line_height * size * lines.len() as f32).max(1.0));
		if scale >= 0.999 {
			break;
		}
		size = (size * scale).max(1.0);
		lines = wrap_lines(&bubble.text, size, max_width);
	}

	let pitch = line_height * size;
	let first_center = box_height / 2.0 - pitch * lines.len() as f32 / 2.0 + pitch / 2.0;
	let fill = parse_color(bubble.color.as_deref()).unwrap_or_else(Color::black);
	let stroke =
		parse_color(bubble.stroke_color.as_deref()).filter(|_| bubble.stroke_width_px > 0.0);
	// strokes are centred on the outline, so half of the width shows outside the glyphs
	let radius = (bubble.stroke_width_px * font_scale / 2.0).max(1.0);
	const OUTLINE: [(f32, f32); 8] = [
		(1.0, 0.0),
		(-1.0, 0.0),
		(0.0, 1.0),
		(0.0, -1.0),
		(0.707, 0.707),
		(-0.707, 0.707),
		(0.707, -0.707),
		(-0.707, -0.707),
	];

	for (i, line) in lines.iter().enumerate() {
		let line_width = measure(line, size).min(box_width);
		let x = left
			+ match bubble.text_align.as_deref() {
				Some("left") => 0.0,
				Some("right") => box_width - line_width,
				_ => (box_width - line_width) / 2.0,
			};
		// draw_text takes the top of the line; glyphs sit about 0.6 em above their centre
		let y = top + first_center + i as f32 * pitch - size * 0.6;
		if let Some(stroke) = stroke {
			for (dx, dy) in OUTLINE {
				let pos = Point::new(x + dx * radius, y + dy * radius);
				canvas.draw_text(line, size, &pos, font, &stroke);
			}
		}
		canvas.draw_text(line, size, &Point::new(x, y), font, &fill);
	}
}

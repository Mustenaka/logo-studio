use base64::{engine::general_purpose, Engine as _};
use image::{DynamicImage, GenericImageView, ImageFormat, Rgba};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use crate::sam2;

#[derive(Debug, Deserialize, Clone)]
pub struct SegPoint {
    pub x: i32,
    pub y: i32,
    pub label: i32, // 1 = foreground, 0 = background
}

#[derive(Debug, Serialize)]
pub struct SegmentResult {
    pub success: bool,
    pub mask: String,
    pub error: Option<String>,
    pub method: String,
}

#[tauri::command]
pub fn segment_image(
    image_src: String,
    points: Vec<SegPoint>,
    mode: String,
    tolerance: Option<u32>,
    sam2_threshold: Option<f32>,  // mask binarisation threshold (default 0.35)
    matte_radius: Option<u32>,    // alpha-matting edge band in pixels (default 8)
) -> SegmentResult {
    let tol = tolerance.unwrap_or(60).clamp(0, 200);
    let threshold = sam2_threshold.unwrap_or(0.50).clamp(0.05, 0.95);
    let radius    = matte_radius.unwrap_or(8).clamp(1, 30);
    match run_segment(&image_src, &points, &mode, tol, threshold, radius) {
        Ok((mask_b64, method)) => SegmentResult { success: true, mask: mask_b64, error: None, method },
        Err(e) => SegmentResult { success: false, mask: String::new(), error: Some(e), method: "error".into() },
    }
}

fn run_segment(image_src: &str, points: &[SegPoint], mode: &str, tolerance: u32, sam2_threshold: f32, matte_radius: u32) -> Result<(String, String), String> {
    let img = decode_data_url(image_src)?;
    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();

    // ══════════════════════════════════════════════════════════════════════
    // PRIMARY: SAM2 (when models are installed)
    //   Always try SAM2 first — it handles any image type correctly.
    //   Alpha matting refines the binary SAM2 mask into sub-pixel edges.
    // ══════════════════════════════════════════════════════════════════════
    let available = sam2::is_available();
    eprintln!("[SEG] sam2_available={available}");

    if available {
        let sam_points: Vec<(f32, f32, i32)> = if mode == "point" && !points.is_empty() {
            let mut pts: Vec<(f32, f32, i32)> =
                points.iter().map(|p| (p.x as f32, p.y as f32, p.label)).collect();
            // Automatically add corner background points when user only gave
            // foreground points — this is critical for SAM2 quality on logos.
            if !pts.iter().any(|&(_, _, l)| l == 0) {
                pts.extend_from_slice(&[
                    (0.0, 0.0, 0), (1023.0, 0.0, 0),
                    (0.0, 1023.0, 0), (1023.0, 1023.0, 0),
                ]);
            }
            pts
        } else {
            // Auto mode: foreground centroid + corner background points
            build_auto_points(&rgba)
        };
        eprintln!("[SEG] SAM2 with {} points  threshold={sam2_threshold:.2}", sam_points.len());
        match sam2::run_sam2(&img, &sam_points, sam2_threshold) {
            Ok(rough) => {
                eprintln!("[SEG] SAM2 OK");
                // SAM2's neural-network output already encodes edge probability.
                // Applying alpha matting on top would mix background colours into
                // edge pixels (causing the dark-halo artefact). Instead we apply
                // a light morphological clean-up: one erosion pass to remove stray
                // single-pixel noise, then encode directly.
                let cleaned = light_cleanup(&rough);
                return encode_result(&rgba, &cleaned, width, height, "sam2");
            }
            Err(e) => eprintln!("[SEG] SAM2 failed: {e}  → classic fallback"),
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // FALLBACK (SAM2 not installed or inference error)
    //   Classic algorithms — no AI, but handle most logo/product images well.
    // ══════════════════════════════════════════════════════════════════════

    // Fallback A: image already has a transparent background
    if mode != "point" && has_transparent_background(&rgba) {
        eprintln!("[SEG] fallback A: transparent bg → alpha passthrough");
        let mask = alpha_to_gray(&rgba);
        return encode_result(&rgba, &mask, width, height, "alpha");
    }

    // Fallback B: solid / near-uniform background (most UI logos)
    if mode != "point" {
        if let Some(bg) = detect_solid_background(&rgba, 28) {
            eprintln!("[SEG] fallback B: solid bg ({},{},{}) → color+matte", bg.0, bg.1, bg.2);
            let rough = color_removal_binary(&rgba, bg, tolerance.max(30));
            let refined = alpha_matte_refine(&rgba, &rough, matte_radius);
            return encode_result(&rgba, &refined, width, height, "color+matte");
        }
    }

    // Fallback C: flood-fill from user/auto seed points
    eprintln!("[SEG] fallback C: flood-fill");
    let mut rough = image::GrayImage::new(width, height);
    if mode == "point" && !points.is_empty() {
        let fg_pts: Vec<(u32, u32)> = points.iter()
            .filter(|p| p.label == 1)
            .map(|p| {
                let px = ((p.x as f32 / 1024.0) * width as f32).clamp(0.0, (width - 1) as f32) as u32;
                let py = ((p.y as f32 / 1024.0) * height as f32).clamp(0.0, (height - 1) as f32) as u32;
                (px, py)
            })
            .collect();
        flood_fill_mask(&rgba, &mut rough, &fg_pts, tolerance)?;
    } else {
        let (cx, cy) = (width / 2, height / 2);
        let seeds = vec![
            (cx, cy),
            (cx.saturating_sub(width / 6), cy),
            ((cx + width / 6).min(width - 1), cy),
            (cx, cy.saturating_sub(height / 6)),
            (cx, (cy + height / 6).min(height - 1)),
        ];
        flood_fill_mask(&rgba, &mut rough, &seeds, tolerance)?;
    }
    let refined = alpha_matte_refine(&rgba, &rough, matte_radius.min(6));
    encode_result(&rgba, &refined, width, height, "flood_fill+matte")
}

// ─────────────────────────────────────────────────────────────────────────────
// Alpha matting  (BFS nearest-color approach, O(w·h) time)
//
// Trimap:
//   - definite foreground = eroded(rough_mask, radius)
//   - definite background = NOT dilated(rough_mask, radius)
//   - unknown zone        = dilated \ eroded  (the edge band)
//
// For each unknown pixel:
//   F = color of nearest definite-fg pixel  (propagated via BFS)
//   B = color of nearest definite-bg pixel  (propagated via BFS)
//   alpha = dot(C-B, F-B) / |F-B|²          (closed-form matte estimate)
// ─────────────────────────────────────────────────────────────────────────────

fn alpha_matte_refine(
    rgba: &image::RgbaImage,
    rough: &image::GrayImage,
    radius: u32,
) -> image::GrayImage {
    let (w, h) = rgba.dimensions();
    let _size = (w * h) as usize;

    let eroded  = fast_erode(rough, radius);
    let dilated = fast_dilate(rough, radius);

    // BFS: propagate nearest definite-fg color into unknown zone
    let fg_colors = bfs_propagate(rgba, &eroded, &dilated, true);
    // BFS: propagate nearest definite-bg color into unknown zone
    let bg_colors = bfs_propagate(rgba, &eroded, &dilated, false);

    let mut result = image::GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let in_eroded  = eroded .get_pixel(x, y)[0] >= 128;
            let in_dilated = dilated.get_pixel(x, y)[0] >= 128;

            let alpha = if in_eroded {
                255u8
            } else if !in_dilated {
                0u8
            } else {
                // Unknown zone
                let p = rgba.get_pixel(x, y);
                let c = [p[0] as f32, p[1] as f32, p[2] as f32];
                let f = fg_colors[idx].unwrap_or(c);
                let b = bg_colors[idx].unwrap_or([255.0, 255.0, 255.0]);
                (solve_alpha(c, f, b) * 255.0) as u8
            };
            result.put_pixel(x, y, image::Luma([alpha]));
        }
    }
    result
}

/// BFS outward from seed pixels (definite fg or bg) into the unknown zone.
/// Returns per-pixel nearest color (None for pixels not reached).
fn bfs_propagate(
    rgba: &image::RgbaImage,
    eroded: &image::GrayImage,
    dilated: &image::GrayImage,
    from_fg: bool,    // true → seed from eroded-fg; false → seed from non-dilated-bg
) -> Vec<Option<[f32; 3]>> {
    let (w, h) = rgba.dimensions();
    let size = (w * h) as usize;
    let mut colors: Vec<Option<[f32; 3]>> = vec![None; size];
    let mut visited = vec![false; size];
    let mut queue = std::collections::VecDeque::new();

    // Seed
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let seed = if from_fg {
                eroded.get_pixel(x, y)[0] >= 128
            } else {
                dilated.get_pixel(x, y)[0] < 128
            };
            if seed {
                let p = rgba.get_pixel(x, y);
                colors[idx] = Some([p[0] as f32, p[1] as f32, p[2] as f32]);
                visited[idx] = true;
                queue.push_back((x, y));
            }
        }
    }

    // BFS: only enter unknown zone (inside dilated but outside eroded)
    let dirs = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)];
    while let Some((x, y)) = queue.pop_front() {
        let src_idx = (y * w + x) as usize;
        let src_color = colors[src_idx];
        for &(dx, dy) in &dirs {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
            let (nx, ny) = (nx as u32, ny as u32);
            let nidx = (ny * w + nx) as usize;
            if visited[nidx] { continue; }
            // Only propagate into the unknown zone (in dilated, not in eroded)
            let in_unknown = dilated.get_pixel(nx, ny)[0] >= 128
                          && eroded .get_pixel(nx, ny)[0] < 128;
            if !in_unknown { continue; }
            visited[nidx] = true;
            colors[nidx] = src_color;
            queue.push_back((nx, ny));
        }
    }
    colors
}

/// Closed-form alpha: alpha = dot(C-B, F-B) / |F-B|²
#[inline]
fn solve_alpha(c: [f32; 3], f: [f32; 3], b: [f32; 3]) -> f32 {
    let fb = [f[0]-b[0], f[1]-b[1], f[2]-b[2]];
    let cb = [c[0]-b[0], c[1]-b[1], c[2]-b[2]];
    let dot  = cb[0]*fb[0] + cb[1]*fb[1] + cb[2]*fb[2];
    let norm = fb[0]*fb[0] + fb[1]*fb[1] + fb[2]*fb[2];
    if norm < 1.0 { return 0.5; }
    (dot / norm).clamp(0.0, 1.0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Separable morphological erosion / dilation  O(w·h·r)
// ─────────────────────────────────────────────────────────────────────────────

fn fast_erode(mask: &image::GrayImage, radius: u32) -> image::GrayImage {
    let (w, h) = mask.dimensions();
    let r = radius as i32;
    // Horizontal min
    let mut tmp = image::GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut v = 255u8;
            for dx in -r..=r {
                let nx = x as i32 + dx;
                if nx < 0 || nx >= w as i32 { v = 0; break; }
                let pv = mask.get_pixel(nx as u32, y)[0];
                if pv < v { v = pv; if v == 0 { break; } }
            }
            tmp.put_pixel(x, y, image::Luma([v]));
        }
    }
    // Vertical min
    let mut out = image::GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut v = 255u8;
            for dy in -r..=r {
                let ny = y as i32 + dy;
                if ny < 0 || ny >= h as i32 { v = 0; break; }
                let pv = tmp.get_pixel(x, ny as u32)[0];
                if pv < v { v = pv; if v == 0 { break; } }
            }
            out.put_pixel(x, y, image::Luma([v]));
        }
    }
    out
}

fn fast_dilate(mask: &image::GrayImage, radius: u32) -> image::GrayImage {
    let (w, h) = mask.dimensions();
    let r = radius as i32;
    // Horizontal max
    let mut tmp = image::GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut v = 0u8;
            for dx in -r..=r {
                let nx = x as i32 + dx;
                if nx < 0 || nx >= w as i32 { continue; }
                let pv = mask.get_pixel(nx as u32, y)[0];
                if pv > v { v = pv; if v == 255 { break; } }
            }
            tmp.put_pixel(x, y, image::Luma([v]));
        }
    }
    // Vertical max
    let mut out = image::GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut v = 0u8;
            for dy in -r..=r {
                let ny = y as i32 + dy;
                if ny < 0 || ny >= h as i32 { continue; }
                let pv = tmp.get_pixel(x, ny as u32)[0];
                if pv > v { v = pv; if v == 255 { break; } }
            }
            out.put_pixel(x, y, image::Luma([v]));
        }
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Level-1: alpha passthrough
// ─────────────────────────────────────────────────────────────────────────────

fn has_transparent_background(rgba: &image::RgbaImage) -> bool {
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 { return false; }
    let samples = [
        (0,0),(w-1,0),(0,h-1),(w-1,h-1),
        (w/2,0),(w/2,h-1),(0,h/2),(w-1,h/2),
        (w/4,0),(3*w/4,0),(w/4,h-1),(3*w/4,h-1),
        (0,h/4),(0,3*h/4),(w-1,h/4),(w-1,3*h/4),
    ];
    let transparent = samples.iter().filter(|&&(x,y)| rgba.get_pixel(x,y)[3] < 30).count();
    transparent >= samples.len() / 2
}

fn alpha_to_gray(rgba: &image::RgbaImage) -> image::GrayImage {
    let (w, h) = rgba.dimensions();
    image::GrayImage::from_fn(w, h, |x,y| image::Luma([rgba.get_pixel(x,y)[3]]))
}

// ─────────────────────────────────────────────────────────────────────────────
// Level-2: solid background detection + binary removal
// ─────────────────────────────────────────────────────────────────────────────

fn detect_solid_background(rgba: &image::RgbaImage, threshold: u32) -> Option<(u8, u8, u8)> {
    let (w, h) = rgba.dimensions();
    if w < 4 || h < 4 { return None; }
    let step_w = (w / 8).max(1);
    let step_h = (h / 8).max(1);
    let mut samples: Vec<(u8,u8,u8)> = Vec::new();
    let mut add = |x: u32, y: u32| {
        let p = rgba.get_pixel(x, y);
        if p[3] > 200 { samples.push((p[0], p[1], p[2])); }
    };
    for x in (0..w).step_by(step_w as usize) { add(x,0); add(x,h-1); }
    for y in (0..h).step_by(step_h as usize) { add(0,y); add(w-1,y); }
    if samples.len() < 4 { return None; }
    let n = samples.len() as u64;
    let mr = samples.iter().map(|s| s.0 as u64).sum::<u64>() / n;
    let mg = samples.iter().map(|s| s.1 as u64).sum::<u64>() / n;
    let mb = samples.iter().map(|s| s.2 as u64).sum::<u64>() / n;
    let vr = samples.iter().map(|s| (s.0 as i64 - mr as i64).pow(2)).sum::<i64>() / n as i64;
    let vg = samples.iter().map(|s| (s.1 as i64 - mg as i64).pow(2)).sum::<i64>() / n as i64;
    let vb = samples.iter().map(|s| (s.2 as i64 - mb as i64).pow(2)).sum::<i64>() / n as i64;
    if (vr as f64).sqrt() as u32 <= threshold
    && (vg as f64).sqrt() as u32 <= threshold
    && (vb as f64).sqrt() as u32 <= threshold {
        Some((mr as u8, mg as u8, mb as u8))
    } else {
        None
    }
}

/// Hard binary mask: 255 = foreground (far from bg color), 0 = background.
/// Matting handles the transition zone, so we use a sharper threshold here.
fn color_removal_binary(
    rgba: &image::RgbaImage,
    bg_color: (u8,u8,u8),
    tolerance: u32,
) -> image::GrayImage {
    let (w, h) = rgba.dimensions();
    image::GrayImage::from_fn(w, h, |x, y| {
        let p = rgba.get_pixel(x, y);
        if p[3] < 10 { return image::Luma([0u8]); }
        let dist = color_dist(
            p[0] as u64, p[1] as u64, p[2] as u64,
            bg_color.0 as u64, bg_color.1 as u64, bg_color.2 as u64,
        );
        image::Luma([if dist > tolerance as u64 { 255u8 } else { 0u8 }])
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Level-3 helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build SAM2 auto points: detect foreground bounding box via background colour,
/// then emit 5 foreground points spread across it + 4 confirmed background corners.
/// Lightweight post-processing for SAM2 binary masks.
/// Removes isolated single-pixel noise with a 1-px erosion followed by
/// a 1-px dilation (opening), which preserves shape while killing specks.
fn light_cleanup(mask: &image::GrayImage) -> image::GrayImage {
    fast_dilate(&fast_erode(mask, 1), 1)
}

fn build_auto_points(rgba: &image::RgbaImage) -> Vec<(f32, f32, i32)> {
    let (w, h) = rgba.dimensions();

    // Detect background colour (fallback: white)
    let bg = detect_solid_background(rgba, 40).unwrap_or((255, 255, 255));
    let bg_tol = 45u64;

    // Compute tight bounding box of foreground (non-background) pixels
    let mut min_x = w; let mut min_y = h;
    let mut max_x = 0u32; let mut max_y = 0u32;
    let mut found = false;
    for y in 0..h {
        for x in 0..w {
            let p = rgba.get_pixel(x, y);
            if p[3] < 30 { continue; }
            let dist = color_dist(
                p[0] as u64, p[1] as u64, p[2] as u64,
                bg.0 as u64, bg.1 as u64, bg.2 as u64,
            );
            if dist > bg_tol {
                if x < min_x { min_x = x; }
                if y < min_y { min_y = y; }
                if x > max_x { max_x = x; }
                if y > max_y { max_y = y; }
                found = true;
            }
        }
    }
    if !found || max_x <= min_x || max_y <= min_y {
        // Fallback: inner quarter of image
        min_x = w / 4; min_y = h / 4;
        max_x = 3 * w / 4; max_y = 3 * h / 4;
    }

    // Map bounding-box coords to 0-1024 canvas space
    let cx_f = |px: u32| -> f32 { px as f32 / w as f32 * 1024.0 };
    let cy_f = |py: u32| -> f32 { py as f32 / h as f32 * 1024.0 };

    let cx = cx_f((min_x + max_x) / 2);
    let cy = cy_f((min_y + max_y) / 2);
    let qw = (max_x.saturating_sub(min_x)) / 4;
    let qh = (max_y.saturating_sub(min_y)) / 4;

    // 5 foreground points: centre + inner-quarter positions
    let mut pts = vec![
        (cx, cy, 1),
        (cx_f(min_x + qw),     cy_f(min_y + qh),     1),
        (cx_f(max_x - qw),     cy_f(min_y + qh),     1),
        (cx_f(min_x + qw),     cy_f(max_y - qh),     1),
        (cx_f(max_x - qw),     cy_f(max_y - qh),     1),
    ];

    // 4 background points at image corners (always background)
    pts.extend_from_slice(&[
        (0.0,    0.0,    0),
        (1023.0, 0.0,    0),
        (0.0,    1023.0, 0),
        (1023.0, 1023.0, 0),
    ]);

    pts
}

fn flood_fill_mask(
    rgba: &image::RgbaImage,
    mask: &mut image::GrayImage,
    seeds: &[(u32, u32)],
    tolerance: u32,
) -> Result<(), String> {
    let (w, h) = rgba.dimensions();
    let mut visited = vec![false; (w * h) as usize];
    let mut queue = std::collections::VecDeque::new();
    for &(sx, sy) in seeds {
        let idx = (sy * w + sx) as usize;
        if !visited[idx] { visited[idx] = true; queue.push_back((sx, sy)); }
    }
    let seed_color = {
        let (mut r, mut g, mut b) = (0u64, 0u64, 0u64);
        for &(sx, sy) in seeds {
            let p = rgba.get_pixel(sx, sy);
            r += p[0] as u64; g += p[1] as u64; b += p[2] as u64;
        }
        let n = seeds.len() as u64;
        (r/n, g/n, b/n)
    };
    let dirs = [(-1i32,0i32),(1,0),(0,-1),(0,1)];
    while let Some((x, y)) = queue.pop_front() {
        mask.put_pixel(x, y, image::Luma([255u8]));
        for &(dx, dy) in &dirs {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
            let (nx, ny) = (nx as u32, ny as u32);
            let nidx = (ny * w + nx) as usize;
            if visited[nidx] { continue; }
            let p = rgba.get_pixel(nx, ny);
            if p[3] < 10 { continue; }
            if color_dist(p[0] as u64, p[1] as u64, p[2] as u64, seed_color.0, seed_color.1, seed_color.2) <= tolerance as u64 {
                visited[nidx] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

fn encode_result(
    rgba: &image::RgbaImage,
    mask: &image::GrayImage,
    width: u32,
    height: u32,
    method: &str,
) -> Result<(String, String), String> {
    let mut result = image::RgbaImage::new(width, height);
    for y in 0..height { for x in 0..width {
        let m = mask.get_pixel(x, y)[0];
        let o = rgba.get_pixel(x, y);
        result.put_pixel(x, y, Rgba([o[0], o[1], o[2], m]));
    }}
    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(result)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("PNG encode: {e}"))?;
    Ok((general_purpose::STANDARD.encode(buf.into_inner()), method.to_string()))
}

fn decode_data_url(data_url: &str) -> Result<DynamicImage, String> {
    let b64 = if data_url.starts_with("data:") {
        data_url.split(',').nth(1).ok_or("Invalid data URL")?
    } else { data_url };
    let bytes = general_purpose::STANDARD.decode(b64).map_err(|e| format!("Base64: {e}"))?;
    image::load_from_memory(&bytes).map_err(|e| format!("Image decode: {e}"))
}

fn color_dist(r1: u64, g1: u64, b1: u64, r2: u64, g2: u64, b2: u64) -> u64 {
    let dr = r1.abs_diff(r2);
    let dg = g1.abs_diff(g2);
    let db = b1.abs_diff(b2);
    (dr*dr + dg*dg + db*db).isqrt()
}

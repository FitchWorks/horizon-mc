//! "RUST EDITION" under the classic title screen's logo.
//!
//! Vanilla's `textures/gui/title/edition.png` reads "JAVA EDITION". Horizon
//! never ships Mojang's art, so the banner is rebuilt at load time from the
//! player's own texture: "EDITION" is kept column for column, and R, U, S and T
//! are Horizon's own 16x16 texel-grid glyphs (MIT, the rectangles below),
//! textured procedurally as grooved iron with weathered rust streaks and drips,
//! then shaded the way the texture's letters are built (a face with darker
//! sides and a light bottom edge, a dark extrusion under it that leans toward
//! the banner's vanishing point, a black outline and a soft rim) in the shades
//! sampled from the texture itself. Only vanilla 26.3's exact texture is
//! rebuilt: a resource pack's edition.png stays as its author drew it.

use std::collections::{HashMap, VecDeque};
use std::ops::Range;

/// The atlas sprite.
pub const SPRITE: &str = "gui/title/edition";
const WIDTH: usize = 512;
const HEIGHT: usize = 64;
/// FNV-1a over the size and RGBA pixels of 26.3's edition.png.
const VANILLA: u64 = 0x4249_0538_6dfb_181f;
/// "EDITION", outline included, starts at this column; left of it is "JAVA".
const EDITION_START: usize = 204;
/// Where the texture's letters put their faces, and how deep they are drawn.
const FACE_TOP: usize = 4;
const FACE_HEIGHT: usize = 39;
const EXTRUSION: usize = 8;
const OUTLINE: usize = 3;
/// The texture's letters are drawn in perspective: the extrusion's bottom row
/// sits `LEAN * (VANISH_X - x)` texels to the side of the face column `x` it
/// comes from, growing evenly with depth. Measured on 26.3's texture: "J"'s
/// extrusion leans 3 to 4 texels right, "D" and "I" not at all, "N" 2 to 3 left.
const VANISH_X: f64 = 280.0;
const LEAN: f64 = 0.013;
/// The letter faces themselves also tilt toward `VANISH_X` from baseline to cap
/// height (mirroring "EDITION"'s far-right "N" and vanilla's old "J"); without
/// this, orthographic faces look pasted onto a perspective banner.
const PERSPECTIVE: f64 = 0.00080;
/// The face-to-face spacing between letters.
const LETTER_GAP: usize = 6;
/// The new glyphs' face width: "RUST" then spans "JAVA"'s columns to within
/// a texel, so "EDITION" stays where it is and the banner stays centred.
const GLYPH_WIDTH: usize = 42;
/// 1 px narrower than vanilla's 39 px "T" so the perspective-tilted top bar
/// and its 3 px outline don't photobomb the open word gap before "EDITION".
const T_WIDTH: usize = 38;
/// The first face column, as "JAVA"'s.
const FIRST_FACE: usize = 4;
/// Where the "T" starts: one letter gap after the S, as "EDITION"'s "T" sat
/// there before Horizon drew its own.
const T_LEFT: usize = FIRST_FACE + 3 * (GLYPH_WIDTH + LETTER_GAP) + 1;

/// Both the glyph geometry and the corrugated iron/rust surface live on the
/// same 16x16 texel grid per letter (4-texel strokes, 3-texel corner/waist
/// notches, 3-texel letter gaps). Keeping font and surface on one grid keeps
/// rust seams locked to the strokes instead of swimming at a different scale.
const TEX_SIZE: usize = 16;
const TEX_GAP: usize = 3;
const TEX_WIDTH: usize = TEX_SIZE * 4 + TEX_GAP * 3;
const RUST_SEED: u64 = 101;

/// Horizon's R, U, S and T on the 16x16 texel grid. R pinches in 3 texels at
/// the waist and drops a straight 4-texel right leg (a diagonal staircase read
/// more like a wheelchair ramp once the horizontal iron seams went across it).
const R: &[[usize; 4]] = &[
    [0, 0, 13, 3],
    [0, 3, 16, 4],
    [0, 4, 4, 6],
    [12, 4, 16, 6],
    [0, 6, 16, 7],
    [0, 7, 13, 9],
    [0, 9, 16, 10],
    [0, 10, 4, 16],
    [12, 10, 16, 16],
];
const U: &[[usize; 4]] = &[
    [0, 0, 4, 12],
    [12, 0, 16, 12],
    [0, 12, 16, 13],
    [3, 13, 13, 16],
];
const S: &[[usize; 4]] = &[
    [3, 0, 16, 3],
    [0, 3, 16, 4],
    [0, 4, 4, 6],
    [0, 6, 13, 7],
    [3, 7, 13, 9],
    [3, 9, 16, 10],
    [12, 10, 16, 12],
    [0, 12, 16, 13],
    [0, 13, 13, 16],
];
const T: &[[usize; 4]] = &[[0, 0, 16, 4], [6, 4, 10, 16]];

/// The banner rebuilt to read "RUST EDITION", or `None` when the texture is
/// not vanilla's (a resource pack's own).
pub fn rust_edition(width: u32, height: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    (fingerprint(width, height, pixels) == VANILLA).then(|| compose(pixels))
}

fn fingerprint(width: u32, height: u32, pixels: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let bytes = width.to_le_bytes().into_iter().chain(height.to_le_bytes());
    for b in bytes.chain(pixels.iter().copied()) {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

type Rgba = [u8; 4];

fn lum(p: Rgba) -> u32 {
    (u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])) / 3
}

fn pixel(pixels: &[u8], x: usize, y: usize) -> Rgba {
    let i = (y * WIDTH + x) * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// The texture's shades: the most common colour in each brightness band.
struct Palette {
    highlight: Rgba,
    face: Rgba,
    rim: Rgba,
    extrusion: Rgba,
    edge: Rgba,
    outline: Rgba,
    soft: Rgba,
}

impl Palette {
    fn sample(pixels: &[u8]) -> Self {
        let mut bands: [HashMap<Rgba, u32>; 7] = Default::default();
        for &p in pixels.as_chunks::<4>().0 {
            let band = match (p[3], lum(p)) {
                (0, _) => continue,
                (255, 215..) => 0,
                (255, 170..) => 1,
                (255, 100..) => 2,
                (255, 45..) => 3,
                (255, 20..) => 4,
                (255, _) => 5,
                _ => 6,
            };
            *bands[band].entry(p).or_default() += 1;
        }
        let mode = |band: &HashMap<Rgba, u32>, fallback: Rgba| {
            band.iter()
                .max_by_key(|&(p, n)| (*n, std::cmp::Reverse(*p)))
                .map_or(fallback, |(p, _)| *p)
        };
        let outline = mode(&bands[5], [0, 0, 0, 255]);
        let face = mode(&bands[1], [200, 200, 200, 255]);
        Self {
            highlight: mode(&bands[0], face),
            face,
            rim: mode(&bands[2], face),
            extrusion: mode(&bands[3], outline),
            edge: mode(&bands[4], outline),
            outline,
            soft: mode(&bands[6], outline),
        }
    }
}

fn lerp_rgba(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let mix = |ca: u8, cb: u8| -> u8 {
        (f32::from(ca) + (f32::from(cb) - f32::from(ca)) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2]), a[3]]
}

/// Integer hash and 4-texel 1D value noise (`0..=255`). Full 2D float fBm was
/// overkill for a 73x16 grid and tended to produce cloudy blobs instead of
/// crisp pixel-art streaks.
fn hash8(x: usize, y: usize, seed: u64) -> u8 {
    let mut h = (x as u64)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u64).wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(1_442_695_040_888_963_407));
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((h ^ (h >> 31)) & 0xff) as u8
}

fn noise8(x: usize, y: usize, seed: u64) -> u8 {
    let i = x / 4;
    let f = (x % 4) as u16;
    let a = u16::from(hash8(i, y, seed));
    let b = u16::from(hash8(i + 1, y, seed));
    ((a * (4 - f) + b * f) / 4) as u8
}

/// Rust doesn't move, so animating it per frame is out, but seeding once from
/// the clock at texture load (~20 ns) gives each launch its own weathering.
/// Pinned in tests so CI doesn't gamble.
fn runtime_seed() -> u64 {
    #[cfg(test)]
    {
        RUST_SEED
    }
    #[cfg(not(test))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(RUST_SEED, |d| (d.as_nanos() as u64) ^ RUST_SEED)
    }
}

struct RustTexture {
    colors: [Rgba; TEX_WIDTH * TEX_SIZE],
    rust: [u8; TEX_WIDTH * TEX_SIZE],
}

impl RustTexture {
    fn generate(mask: &[bool; TEX_WIDTH * TEX_SIZE], palette: &Palette, seed: u64) -> Self {
        // Hybrid stone/iron palette: raw exposed_iron greys are too bright and
        // cold next to "EDITION"'s warm stone (#d0c5c0). Deriving the 4 iron
        // shades by lerping between "EDITION"'s sampled face, highlight and rim
        // makes "RUST" look forged from the same material, while the 4 oxidized
        // rust shades stay fixed so rust still reads as rust.
        let iron_hi = lerp_rgba(palette.face, palette.highlight, 0.30);
        let iron_base = palette.face;
        let iron_mid = lerp_rgba(palette.face, palette.rim, 0.40);
        let iron_dark = lerp_rgba(palette.face, palette.rim, 0.68);

        let rust_shades: [Rgba; 4] = [
            [195, 115, 78, 255],
            [180, 89, 47, 255],
            [156, 69, 30, 255],
            [132, 72, 47, 255],
        ];
        let stain_base = lerp_rgba(iron_base, rust_shades[0], 0.45);
        let stain_hi = lerp_rgba(iron_hi, rust_shades[0], 0.35);

        // Corrugated iron seams sit on `y % 3 == 0` (0, 3, 6, 9, 12, 15) so both
        // the top and bottom edges land on a dark groove (starting at row 1 gave
        // a double-groove up top and left the baseline naked). Horizontal rust
        // striations only make visual sense when anchored inside those seams.
        let mut rust = [0u8; TEX_WIDTH * TEX_SIZE];
        for y in (0..TEX_SIZE).step_by(3) {
            let sy = y + 1;
            for x in 0..TEX_WIDTH {
                if !mask[y * TEX_WIDTH + x] {
                    continue;
                }
                let n = noise8(x, sy, seed).max(if y == 0 { noise8(x, 0, seed) } else { 0 });
                if n > 112 {
                    let lvl = (2 + (u16::from(n - 112) * 4 / 143) as u8).min(5);
                    rust[y * TEX_WIDTH + x] = lvl;
                    for dx in [-1isize, 1] {
                        let nx = x as isize + dx;
                        if (0..TEX_WIDTH as isize).contains(&nx)
                            && mask[y * TEX_WIDTH + nx as usize]
                        {
                            let nidx = y * TEX_WIDTH + nx as usize;
                            rust[nidx] = rust[nidx].max(1);
                        }
                    }
                }
            }
        }

        // Gravity drips spawn from core/deep groove rust (levels 4..=5), flare
        // sideways on step 1 for a conical head, and occasionally jog 1 texel
        // left or right as they cross the highlight bevel and plate face below.
        for y in (0..TEX_SIZE - 1).step_by(3) {
            let sy = y + 1;
            for x in 0..TEX_WIDTH {
                let lvl = rust[y * TEX_WIDTH + x];
                if lvl < 4 || hash8(x, sy, seed ^ 0x1f7) >= 58 {
                    continue;
                }
                let dlen = if hash8(x, sy, seed ^ 0x1fd) < 160 {
                    2
                } else {
                    3
                };
                let mut cx = x;
                for s in 1..=dlen {
                    let cy = y + s;
                    if cy >= TEX_SIZE || !mask[cy * TEX_WIDTH + cx] {
                        break;
                    }
                    let dlvl = lvl.saturating_sub(s as u8).max(2);
                    rust[cy * TEX_WIDTH + cx] = rust[cy * TEX_WIDTH + cx].max(dlvl);
                    if s == 1 {
                        for dx in [-1isize, 1] {
                            let nx = cx as isize + dx;
                            if (0..TEX_WIDTH as isize).contains(&nx)
                                && mask[cy * TEX_WIDTH + nx as usize]
                                && hash8(nx as usize, cy, seed ^ 0x259) < 180
                            {
                                let nidx = cy * TEX_WIDTH + nx as usize;
                                rust[nidx] = rust[nidx].max(dlvl.saturating_sub(1).max(1));
                            }
                        }
                    }
                    if hash8(cx, cy, seed ^ (0x2c5 + s as u64)) < 82 {
                        let step = if hash8(cx, cy, seed ^ 0x2cf) < 128 {
                            1
                        } else {
                            -1
                        };
                        let nx = cx as isize + step;
                        if (0..TEX_WIDTH as isize).contains(&nx)
                            && mask[cy * TEX_WIDTH + nx as usize]
                        {
                            cx = nx as usize;
                        }
                    }
                }
            }
        }

        let mut colors = [[0, 0, 0, 0]; TEX_WIDTH * TEX_SIZE];
        for y in 0..TEX_SIZE {
            let sy = y + 1;
            for x in 0..TEX_WIDTH {
                let idx = y * TEX_WIDTH + x;
                if !mask[idx] {
                    continue;
                }
                let is_hi = y % 3 == 1 && noise8(x, sy + 19, seed) > 92;
                let iron_col = if y % 3 == 0 {
                    if noise8(x, sy, seed ^ 0x55) > 115 {
                        iron_dark
                    } else {
                        iron_mid
                    }
                } else if is_hi {
                    iron_hi
                } else {
                    iron_base
                };
                colors[idx] = match rust[idx] {
                    0 => iron_col,
                    1 => {
                        if is_hi {
                            stain_hi
                        } else {
                            stain_base
                        }
                    }
                    r => rust_shades[(r - 2).min(3) as usize],
                };
            }
        }
        Self { colors, rust }
    }

    fn sample(&self, x: usize, y: usize) -> Option<(Rgba, u8)> {
        let (tx, ty) = banner_to_texel(x as f64 + 0.5, y)?;
        let idx = ty * TEX_WIDTH + tx;
        Some((self.colors[idx], self.rust[idx]))
    }
}

fn glyph_specs() -> [(usize, usize, &'static [[usize; 4]]); 4] {
    [
        (FIRST_FACE, GLYPH_WIDTH, R),
        (FIRST_FACE + GLYPH_WIDTH + LETTER_GAP, GLYPH_WIDTH, U),
        (FIRST_FACE + 2 * (GLYPH_WIDTH + LETTER_GAP), GLYPH_WIDTH, S),
        (T_LEFT, T_WIDTH, T),
    ]
}

fn banner_to_texel(xf: f64, y: usize) -> Option<(usize, usize)> {
    if !(FACE_TOP..FACE_TOP + FACE_HEIGHT).contains(&y) {
        return None;
    }
    let ly = y - FACE_TOP;
    // 39 px isn't divisible by 16 texels: naive `ly * 16 / 39` made some
    // grooves 3 px thick, others 2 px, and the bottom groove 1 px after the
    // 1 px bottom specular highlight ate half of it (and box-filtering fixed
    // the thickness by turning crisp pixel art into blurry soup). Stepping
    // in 7 px strips (2 px groove, 2 px highlight, 3 px base, with 36..38
    // for bottom groove 15 and 38 for the specular edge) keeps every groove
    // an exact, crisp 2 px.
    let ty = if ly >= 36 {
        15
    } else {
        (ly / 7) * 3 + (ly % 7).min(5) / 2
    };
    let dy = (FACE_TOP + FACE_HEIGHT - 1 - y) as f64;
    let ox = VANISH_X + (xf - VANISH_X) * (1.0 + dy * PERSPECTIVE);
    if ox < 0.0 {
        return None;
    }
    for (i, (left, width, _)) in glyph_specs().into_iter().enumerate() {
        if ox >= left as f64 && ox < (left + width) as f64 {
            let lx = (((ox - left as f64) * TEX_SIZE as f64) / width as f64) as usize;
            let tx = i * (TEX_SIZE + TEX_GAP) + lx.min(TEX_SIZE - 1);
            return Some((tx, ty));
        }
    }
    None
}

fn texel_mask() -> [bool; TEX_WIDTH * TEX_SIZE] {
    let mut mask = [false; TEX_WIDTH * TEX_SIZE];
    for (i, (_, _, rects)) in glyph_specs().into_iter().enumerate() {
        let ox = i * (TEX_SIZE + TEX_GAP);
        for &[x0, y0, x1, y1] in rects {
            for y in y0..y1 {
                for x in ox + x0..ox + x1 {
                    mask[y * TEX_WIDTH + x] = true;
                }
            }
        }
    }
    mask
}

/// Faces of the new glyphs, positioned in the texture.
fn glyph_faces(mask: &[bool; TEX_WIDTH * TEX_SIZE]) -> Vec<bool> {
    let mut face = vec![false; WIDTH * HEIGHT];
    for y in FACE_TOP..FACE_TOP + FACE_HEIGHT {
        for x in 0..EDITION_START {
            if let Some((tx, ty)) = banner_to_texel(x as f64 + 0.5, y) {
                face[y * WIDTH + x] = mask[ty * TEX_WIDTH + tx];
            }
        }
    }
    face
}

/// How far the extrusion `depth` texels under face column `x` leans sideways.
fn lean(x: usize, depth: usize) -> isize {
    ((VANISH_X - x as f64) * LEAN * depth as f64 / EXTRUSION as f64).round() as isize
}

/// Draw faces within `columns` the way the texture's letters are drawn.
fn draw_glyphs(
    out: &mut [u8],
    face: &[bool],
    palette: &Palette,
    columns: Range<usize>,
    rust_data: Option<(&[bool; TEX_WIDTH * TEX_SIZE], &RustTexture)>,
) {
    let at = |grid: &[bool], x: isize, y: isize| {
        (columns.start as isize..columns.end as isize).contains(&x)
            && (0..HEIGHT as isize).contains(&y)
            && grid[y as usize * WIDTH + x as usize]
    };
    let mut body = face.to_vec();
    let mut body_rust = vec![0u8; WIDTH * HEIGHT];
    for y in 0..HEIGHT {
        for x in columns.clone() {
            if !face[y * WIDTH + x] {
                continue;
            }
            let r_lvl = rust_data
                .and_then(|(_, tex)| tex.sample(x, y))
                .map_or(0, |(_, lvl)| lvl);
            for depth in 1..=EXTRUSION {
                let bx = x as isize + lean(x, depth);
                let by = y + depth;
                if by < HEIGHT && (columns.start as isize..columns.end as isize).contains(&bx) {
                    let bidx = by * WIDTH + bx as usize;
                    body[bidx] = true;
                    body_rust[bidx] = body_rust[bidx].max(r_lvl);
                }
            }
        }
    }

    // Selective anti-aliasing: the interior iron/rust texture is pixel art and
    // stays blocky, but the perspective-tilted letter silhouettes and 3 px
    // black outline step by 1 px every ~5 rows — fine in the dark, chainsaw
    // against a bright sky. 8x horizontal sub-pixel coverage on just the
    // slanted side rims (`fcov`) and outer outline (`ocov`) gives the smooth
    // alpha ramp vanilla's pre-rendered "JAVA" and "N" have.
    const SUB: usize = 8;
    const W_SUB: usize = EDITION_START * SUB;
    let sub_grids = rust_data.map(|(mask, _)| {
        let mut face_sub = vec![false; W_SUB * HEIGHT];
        let mut body_sub = vec![false; W_SUB * HEIGHT];
        for y in FACE_TOP..FACE_TOP + FACE_HEIGHT {
            for sx in 0..W_SUB {
                if banner_to_texel((sx as f64 + 0.5) / SUB as f64, y)
                    .is_some_and(|(tx, ty)| mask[ty * TEX_WIDTH + tx])
                {
                    face_sub[y * W_SUB + sx] = true;
                    body_sub[y * W_SUB + sx] = true;
                    for depth in 1..=EXTRUSION {
                        let l = ((VANISH_X * SUB as f64 - sx as f64) * LEAN * depth as f64
                            / EXTRUSION as f64)
                            .round() as isize;
                        let bsx = sx as isize + l;
                        let by = y + depth;
                        if by < HEIGHT && (0..W_SUB as isize).contains(&bsx) {
                            body_sub[by * W_SUB + bsx as usize] = true;
                        }
                    }
                }
            }
        }
        let mut horiz_sub = vec![false; W_SUB * HEIGHT];
        let reach_sub = OUTLINE * SUB;
        for y in 0..HEIGHT {
            let row = &body_sub[y * W_SUB..(y + 1) * W_SUB];
            let out_row = &mut horiz_sub[y * W_SUB..(y + 1) * W_SUB];
            let mut dist = reach_sub + 1;
            for sx in 0..W_SUB {
                dist = if row[sx] { 0 } else { dist + 1 };
                if dist <= reach_sub {
                    out_row[sx] = true;
                }
            }
            dist = reach_sub + 1;
            for sx in (0..W_SUB).rev() {
                dist = if row[sx] { 0 } else { dist + 1 };
                if dist <= reach_sub {
                    out_row[sx] = true;
                }
            }
        }
        let mut outline_sub = vec![false; W_SUB * HEIGHT];
        for y in 0..HEIGHT {
            let y0 = y.saturating_sub(OUTLINE);
            let y1 = (y + OUTLINE).min(HEIGHT - 1);
            for sx in 0..W_SUB {
                if (y0..=y1).any(|yy| horiz_sub[yy * W_SUB + sx]) {
                    outline_sub[y * W_SUB + sx] = true;
                }
            }
        }
        (face_sub, outline_sub)
    });

    let reach = OUTLINE as isize + 1;
    for y in 0..HEIGHT {
        for x in columns.clone() {
            let (xi, yi) = (x as isize, y as isize);
            let colour = if at(face, xi, yi) {
                let base_col = rust_data
                    .and_then(|(_, tex)| tex.sample(x, y))
                    .map_or(palette.face, |(c, _)| c);
                let mut fg = if !at(face, xi, yi + 1) {
                    if rust_data.is_some() {
                        lerp_rgba(base_col, palette.highlight, 0.60)
                    } else {
                        palette.highlight
                    }
                } else if !at(face, xi - 1, yi) || !at(face, xi + 1, yi) {
                    if rust_data.is_some() {
                        lerp_rgba(base_col, palette.rim, 0.35)
                    } else {
                        palette.rim
                    }
                } else {
                    base_col
                };
                if (!at(face, xi - 1, yi) || !at(face, xi + 1, yi))
                    && let Some((face_sub, _)) = &sub_grids
                {
                    let fcov = (x * SUB..(x + 1) * SUB)
                        .filter(|&sx| face_sub[y * W_SUB + sx])
                        .count();
                    if fcov > 0 && fcov < SUB {
                        fg = lerp_rgba(palette.extrusion, fg, fcov as f32 / SUB as f32);
                    }
                }
                fg
            } else if at(&body, xi, yi) {
                let rusty_drop = rust_data.is_some() && body_rust[y * WIDTH + x] >= 3;
                if at(face, xi, yi - 1) {
                    if rusty_drop {
                        lerp_rgba(palette.rim, [110, 52, 26, 255], 0.35)
                    } else {
                        palette.rim
                    }
                } else if !at(&body, xi - 1, yi) || !at(&body, xi + 1, yi) || !at(&body, xi, yi + 1)
                {
                    if rusty_drop {
                        lerp_rgba(palette.edge, [54, 24, 10, 255], 0.55)
                    } else {
                        palette.edge
                    }
                } else if rusty_drop {
                    lerp_rgba(palette.extrusion, [69, 40, 28, 255], 0.55)
                } else {
                    palette.extrusion
                }
            } else if let Some((_, outline_sub)) = &sub_grids {
                if y >= 55 || x >= 195 {
                    continue;
                }
                let ocov = (x * SUB..(x + 1) * SUB)
                    .filter(|&sx| outline_sub[y * W_SUB + sx])
                    .count();
                if ocov == SUB {
                    palette.outline
                } else if ocov > 0 {
                    [
                        palette.outline[0],
                        palette.outline[1],
                        palette.outline[2],
                        (ocov * 244 / SUB) as u8,
                    ]
                } else {
                    continue;
                }
            } else {
                let distance = (-reach..=reach)
                    .flat_map(|dy| (-reach..=reach).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| at(&body, xi + dx, yi + dy))
                    .map(|(dx, dy)| dx.abs().max(dy.abs()))
                    .min();
                match distance {
                    Some(d) if d <= OUTLINE as isize => palette.outline,
                    Some(_) => palette.soft,
                    None => continue,
                }
            };
            put(out, x, y, colour);
        }
    }
}

/// Paint `p` over what is there unless that is more opaque, or as opaque and
/// brighter (a letter's body wins over a neighbour's outline).
fn put(out: &mut [u8], x: usize, y: usize, p: Rgba) {
    let old = pixel(out, x, y);
    if (p[3], lum(p)) > (old[3], lum(old)) {
        out[(y * WIDTH + x) * 4..][..4].copy_from_slice(&p);
    }
}

/// Close the pockets left of "EDITION" that no gap reaches from outside, as
/// the texture's letters fill their counters with the outline.
fn fill_counters(out: &mut [u8], outline: Rgba) {
    let open = |out: &[u8], x: usize, y: usize| pixel(out, x, y)[3] < 255;
    let mut seen = vec![false; WIDTH * HEIGHT];
    let mut queue = VecDeque::new();
    for y in 0..HEIGHT {
        for x in 0..EDITION_START {
            let border = x == 0 || y == 0 || x == EDITION_START - 1 || y == HEIGHT - 1;
            if border && open(out, x, y) {
                seen[y * WIDTH + x] = true;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            if !(0..EDITION_START as isize).contains(&nx) || !(0..HEIGHT as isize).contains(&ny) {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if !seen[ny * WIDTH + nx] && open(out, nx, ny) {
                seen[ny * WIDTH + nx] = true;
                queue.push_back((nx, ny));
            }
        }
    }
    for y in 0..HEIGHT {
        for x in 0..EDITION_START {
            if !seen[y * WIDTH + x] && open(out, x, y) {
                out[(y * WIDTH + x) * 4..][..4].copy_from_slice(&outline);
            }
        }
    }
}

/// Give remaining hard outline edges a faint feather. Vanilla's most common
/// semi-transparent pixel (`palette.soft`) is alpha 244 (96% opaque black);
/// painting that on every 8-neighbour just added a second jagged black border.
/// Leaving the `ocov` ramp alone and feathering untouched `alpha == 0`
/// neighbours at alpha 24 keeps the soft rim contract without growing teeth.
fn soften_edges(out: &mut [u8], soft: Rgba) {
    let opaque = |out: &[u8], x: isize, y: isize| {
        (0..EDITION_START as isize).contains(&x)
            && (0..HEIGHT as isize).contains(&y)
            && pixel(out, x as usize, y as usize)[3] == 255
    };
    let feather = [soft[0], soft[1], soft[2], 24];
    let mut rim = Vec::new();
    for y in 0..55 {
        for x in 0..195 {
            let (xi, yi) = (x as isize, y as isize);
            let near = (-1..=1).any(|dy| (-1..=1).any(|dx| opaque(out, xi + dx, yi + dy)));
            if pixel(out, x, y)[3] == 0 && near {
                rim.push((x, y));
            }
        }
    }
    for (x, y) in rim {
        out[(y * WIDTH + x) * 4..][..4].copy_from_slice(&feather);
    }
}

/// "RUST" in place of "JAVA"; `pixels` is a 512x64 RGBA banner laid out as
/// vanilla's.
fn compose(pixels: &[u8]) -> Vec<u8> {
    let palette = Palette::sample(pixels);
    let mask = texel_mask();
    let rust_tex = RustTexture::generate(&mask, &palette, runtime_seed());
    let mut out = pixels.to_vec();
    for row in out.as_chunks_mut::<{ WIDTH * 4 }>().0 {
        row[..EDITION_START * 4].fill(0);
    }
    draw_glyphs(
        &mut out,
        &glyph_faces(&mask),
        &palette,
        0..EDITION_START,
        Some((&mask, &rust_tex)),
    );
    fill_counters(&mut out, palette.outline);
    soften_edges(&mut out, palette.soft);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACE: Rgba = [208, 197, 192, 255];

    /// A banner laid out as vanilla's, in made-up colours: every letter a
    /// solid block over its vanilla columns, with the font's shading.
    fn stand_in() -> Vec<u8> {
        // Vanilla's letter bodies (extrusion edge included) by column.
        let runs = [
            (4, 97),
            (101, 144),
            (147, 190),
            (208, 249),
            (256, 296),
            (303, 330),
            (335, 376),
            (381, 411),
            (413, 507),
        ];
        let mut face = vec![false; WIDTH * HEIGHT];
        for (x0, x1) in runs {
            for y in FACE_TOP..FACE_TOP + 39 {
                for x in x0..=x1 {
                    face[y * WIDTH + x] = true;
                }
            }
        }
        let palette = Palette {
            highlight: [226, 216, 213, 255],
            face: FACE,
            rim: [153, 147, 145, 255],
            extrusion: [61, 57, 56, 255],
            edge: [45, 42, 42, 255],
            outline: [0, 0, 0, 255],
            soft: [0, 0, 0, 244],
        };
        let mut out = vec![0; WIDTH * HEIGHT * 4];
        draw_glyphs(&mut out, &face, &palette, 0..WIDTH, None);
        // A colour only "JAVA" has, to see it go.
        out[(20 * WIDTH + 30) * 4..][..4].copy_from_slice(&[255, 0, 255, 255]);
        out
    }

    fn columns(pixels: &[u8], range: Range<usize>) -> Vec<u8> {
        pixels
            .as_chunks::<{ WIDTH * 4 }>()
            .0
            .iter()
            .flat_map(|row| row[range.start * 4..range.end * 4].to_vec())
            .collect()
    }

    /// Outlines left of "EDITION" keep their soft rim all round.
    fn assert_soft_rim(out: &[u8]) {
        for y in 1..HEIGHT - 1 {
            for x in 1..EDITION_START - 1 {
                if pixel(out, x, y)[3] == 255 {
                    let near = [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)];
                    assert!(
                        near.iter().all(|&(nx, ny)| pixel(out, nx, ny)[3] > 0),
                        "{x},{y}"
                    );
                }
            }
        }
    }

    #[test]
    fn rust_replaces_java_and_edition_is_untouched() {
        let source = stand_in();
        let out = compose(&source);
        assert_eq!(out.len(), WIDTH * HEIGHT * 4);
        assert_eq!(
            columns(&out, EDITION_START..WIDTH),
            columns(&source, EDITION_START..WIDTH)
        );
        assert!(!out.as_chunks::<4>().0.contains(&[255, 0, 255, 255]));
        // R, U and S: their strokes are faces, their notches and counters not.
        let lit = |x: usize, y: usize| lum(pixel(&out, x, y)) >= 75;
        let project_x = |ortho_x: usize, y: usize| -> usize {
            let dy = (FACE_HEIGHT - 1 - y) as f64;
            (VANISH_X + (ortho_x as f64 - VANISH_X) / (1.0 + dy * PERSPECTIVE)).round() as usize
        };
        let glyph = |i: usize, x: usize, y: usize| {
            let ortho_x = FIRST_FACE + i * (GLYPH_WIDTH + LETTER_GAP) + x;
            lit(project_x(ortho_x, y), FACE_TOP + y)
        };
        for (i, shape) in [R, U, S].into_iter().enumerate() {
            for &[x0, y0, x1, y1] in shape {
                let cx = ((x0 + x1) * GLYPH_WIDTH) / (2 * TEX_SIZE);
                let cy = ((y0 + y1) * FACE_HEIGHT) / (2 * TEX_SIZE);
                assert!(glyph(i, cx, cy), "glyph {i} at ({cx}, {cy})");
            }
        }
        assert!(!glyph(0, 38, 19), "R's waist notch between bowl and leg");
        assert!(!glyph(0, 38, 2), "R's top-right corner notch");
        assert!(!glyph(0, 20, 30), "R's open bottom");
        assert!(!glyph(1, 20, 5), "U's open top");
        assert!(!glyph(1, 2, 36), "U's rounded corner");
        assert!(!glyph(2, 2, 22), "S's lower-left notch");
        assert!(!glyph(2, 38, 17), "S's upper-right notch");
        // The T: its bar and stem lit, either side of the stem under the bar not.
        let t = |x: usize, y: usize| lit(project_x(T_LEFT + x, y), FACE_TOP + y);
        assert!(t(2, 5) && t(36, 5) && t(19, 30), "T's bar and stem");
        assert!(!t(5, 30) && !t(33, 30), "T's open sides");
        assert_soft_rim(&out);
        // Nothing below the 14 GUI rows vanilla draws, and the word gap stays open.
        assert!(out[WIDTH * 55 * 4..].iter().all(|&b| b == 0));
        let gap = columns(&out, 195..EDITION_START);
        assert!(gap.as_chunks::<4>().0.iter().all(|p| p[3] == 0));
        // Centred: "RUST" spans "JAVA"'s columns to within a texel.
        let used = |x: usize| (0..HEIGHT).any(|y| pixel(&out, x, y)[3] > 0);
        let last = (0..EDITION_START).rev().find(|&x| used(x)).unwrap();
        assert!(
            used(0) && (192..=194).contains(&last),
            "RUST ends at {last}"
        );
    }

    /// The extrusion leans toward the vanishing point as the texture's letters'
    /// do: a face's left edge far left of it drops its last extrusion row 4
    /// texels right (26.3's "J"), one near it straight down, one far right 2
    /// texels left (its "N").
    fn body_left(out: &[u8], y: usize, mut range: Range<usize>) -> usize {
        range
            .find(|&x| {
                let p = pixel(out, x, y);
                p[3] == 255 && lum(p) >= 20
            })
            .expect("a body in the range")
    }

    #[test]
    fn extrusions_lean_toward_the_vanishing_point() {
        let mut face = vec![false; WIDTH * HEIGHT];
        for (x0, x1) in [(10, 37), (270, 290), (466, 478)] {
            for y in FACE_TOP..FACE_TOP + 39 {
                for x in x0..x1 {
                    face[y * WIDTH + x] = true;
                }
            }
        }
        let palette = Palette::sample(&stand_in());
        let mut out = vec![0; WIDTH * HEIGHT * 4];
        draw_glyphs(&mut out, &face, &palette, 0..WIDTH, None);
        let last = FACE_TOP + 38 + EXTRUSION;
        for (range, left, shift) in [(0..60, 10, 4), (240..320, 270, 0), (440..512, 466, -2)] {
            let edge = body_left(&out, last, range.clone()) as isize;
            assert_eq!(edge - left, shift, "{range:?}");
        }
    }

    #[test]
    fn only_vanilla_is_rebuilt() {
        let source = stand_in();
        assert!(rust_edition(512, 64, &source).is_none());
    }

    /// The R matches the 16x16 texel grid font with vanishing-point tilt:
    /// a 3-texel waist notch and a 4-texel right leg angled toward `VANISH_X`.
    #[test]
    fn r_leg_matches_texel_grid() {
        let out = compose(&stand_in());
        let lit = |x: usize, y: usize| lum(pixel(&out, x, FACE_TOP + y)) >= 75;
        let stem_left = |y: usize| (0..FIRST_FACE + 20).find(|&x| lit(x, y)).expect("the stem");
        let leg_left = |y: usize| {
            (FIRST_FACE + 15..FIRST_FACE + GLYPH_WIDTH + 12)
                .find(|&x| lit(x, y))
                .expect("the leg")
        };
        // Top leans right toward VANISH_X relative to baseline.
        assert!(stem_left(0) > stem_left(38));
        assert_eq!(stem_left(38), FIRST_FACE);
        // Under the middle bar and its 1px extrusion rim (y >= 26), the leg
        // leans monotonically outward to the baseline at x = FIRST_FACE + 31.
        assert!((26..38).all(|y| leg_left(y) >= leg_left(y + 1)));
        assert_eq!(leg_left(38), FIRST_FACE + 31);
        assert!(
            lit(FIRST_FACE + 38, 10) && !lit(FIRST_FACE + 45, 19),
            "bowl and waist notch"
        );
    }

    #[test]
    fn rust_texture_has_iron_grooves_and_rust_tones() {
        let out = compose(&stand_in());
        let mut has_rust = false;
        let mut has_iron = false;
        for y in FACE_TOP..FACE_TOP + FACE_HEIGHT {
            for x in FIRST_FACE..T_LEFT + T_WIDTH {
                let p = pixel(&out, x, y);
                if p[3] == 255 && lum(p) >= 75 {
                    if p[0] > p[2] + 35 {
                        has_rust = true;
                    } else if p[0].abs_diff(p[2]) < 20 {
                        has_iron = true;
                    }
                }
            }
        }
        assert!(has_rust && has_iron);
    }

    /// Against the prepared vanilla resources when they are present.
    #[test]
    fn prepared_vanilla_edition() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.local/horizon-client")
            .join(horizon_core::VANILLA_VERSION)
            .join("assets/minecraft/textures/gui/title/edition.png");
        let Ok((w, h, pixels)) = crate::hud::decode(&path) else {
            return;
        };
        let out = rust_edition(w, h, &pixels).expect("vanilla's edition.png");
        assert_eq!(
            columns(&out, EDITION_START..WIDTH),
            columns(&pixels, EDITION_START..WIDTH)
        );
        assert_eq!(Palette::sample(&pixels).face, FACE);
        assert_soft_rim(&out);
        let mut edited = pixels.clone();
        edited[3] ^= 1;
        assert!(rust_edition(w, h, &edited).is_none());
    }
}

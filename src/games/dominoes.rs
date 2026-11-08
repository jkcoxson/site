// Jackson Coxson
// Dominoes for any dominoes game on the site: the tiles of a double-N set and
// how they're drawn. Tiles are SVG built as strings, so the same markup works
// in a Leptos view (`inner_html`) and in a ghost element flying across the
// page. Each number has its own pip color, like a real double-12 set, so
// matches are easy to spot on a phone.

use serde::{Deserialize, Serialize};

/// One domino. `a <= b`; `id` is unique within a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tile {
    pub id: u8,
    pub a: u8,
    pub b: u8,
}

impl Tile {
    pub fn is_double(&self) -> bool {
        self.a == self.b
    }

    pub fn has(&self, n: u8) -> bool {
        self.a == n || self.b == n
    }

    /// The other end from `n` (`n` itself for a double).
    pub fn other(&self, n: u8) -> u8 {
        if self.a == n {
            self.b
        } else {
            self.a
        }
    }

    pub fn pips(&self) -> i32 {
        self.a as i32 + self.b as i32
    }

    pub fn label(&self) -> String {
        format!("{}-{}", self.a, self.b)
    }
}

/// Every tile from 0-0 up to `top`-`top`, numbered in order.
pub fn full_set(top: u8) -> Vec<Tile> {
    let mut tiles = Vec::new();
    for a in 0..=top {
        for b in a..=top {
            tiles.push(Tile {
                id: tiles.len() as u8,
                a,
                b,
            });
        }
    }
    tiles
}

/// The tiles in a double-`top` set.
pub fn set_size(top: u8) -> usize {
    let n = top as usize + 1;
    n * (n + 1) / 2
}

// ── Drawing ───────────────────────────────────────────────────────────────────

/// Pip color for each number, 1 to 12 (0 has none).
pub fn pip_color(n: u8) -> &'static str {
    match n {
        1 => "#0284c7",
        2 => "#15803d",
        3 => "#dc2626",
        4 => "#92400e",
        5 => "#1d4ed8",
        6 => "#ca8a04",
        7 => "#7e22ce",
        8 => "#0f766e",
        9 => "#db2777",
        10 => "#1e293b",
        11 => "#ea580c",
        _ => "#475569",
    }
}

/// Pip centers for `n` in a 100×100 square.
fn pip_spots(n: u8) -> Vec<(f64, f64)> {
    let (l, c, r) = (24.0, 50.0, 76.0);
    let (t, m, b) = (24.0, 50.0, 76.0);
    // Columns of four, for 10 to 12.
    let four = [17.0, 39.0, 61.0, 83.0];
    match n {
        0 => vec![],
        1 => vec![(c, m)],
        2 => vec![(l, t), (r, b)],
        3 => vec![(l, t), (c, m), (r, b)],
        4 => vec![(l, t), (r, t), (l, b), (r, b)],
        5 => vec![(l, t), (r, t), (c, m), (l, b), (r, b)],
        6 => vec![(l, t), (r, t), (l, m), (r, m), (l, b), (r, b)],
        7 => vec![(l, t), (r, t), (l, m), (c, m), (r, m), (l, b), (r, b)],
        8 => vec![
            (l, t),
            (c, t),
            (r, t),
            (l, m),
            (r, m),
            (l, b),
            (c, b),
            (r, b),
        ],
        9 => vec![
            (l, t),
            (c, t),
            (r, t),
            (l, m),
            (c, m),
            (r, m),
            (l, b),
            (c, b),
            (r, b),
        ],
        _ => {
            let (l, c, r) = (22.0, 50.0, 78.0);
            let mut spots: Vec<(f64, f64)> = four.iter().flat_map(|&y| [(l, y), (r, y)]).collect();
            match n {
                10 => spots.extend([(c, 39.0), (c, 61.0)]),
                11 => spots.extend([(c, 28.0), (c, 50.0), (c, 72.0)]),
                _ => spots.extend(four.iter().map(|&y| (c, y))),
            }
            spots
        }
    }
}

fn pips_svg(out: &mut String, n: u8, dx: f64, dy: f64) {
    let r = if n >= 10 { 7.5 } else { 9.0 };
    let color = pip_color(n);
    for (x, y) in pip_spots(n) {
        out.push_str(&format!(
            r#"<circle cx="{:.1}" cy="{:.1}" r="{r}" fill="{color}"/>"#,
            x + dx,
            y + dy
        ));
    }
}

/// A face-up domino as SVG markup filling its box. Horizontal tiles show
/// `first` on the left; vertical ones show it on top.
pub fn tile_svg(first: u8, second: u8, vertical: bool) -> String {
    let (w, h) = if vertical { (100, 200) } else { (200, 100) };
    let mut s = format!(
        r##"<svg viewBox="0 0 {w} {h}" class="domino-svg" xmlns="http://www.w3.org/2000/svg"><rect x="2" y="2" width="{}" height="{}" rx="14" fill="#fbf8f1" stroke="#a8a29e" stroke-width="3"/>"##,
        w - 4,
        h - 4
    );
    let (dx, dy) = if vertical { (0.0, 100.0) } else { (100.0, 0.0) };
    if vertical {
        s.push_str(
            r##"<line x1="14" y1="100" x2="86" y2="100" stroke="#a8a29e" stroke-width="3"/>"##,
        );
    } else {
        s.push_str(
            r##"<line x1="100" y1="14" x2="100" y2="86" stroke="#a8a29e" stroke-width="3"/>"##,
        );
    }
    pips_svg(&mut s, first, 0.0, 0.0);
    pips_svg(&mut s, second, dx, dy);
    s.push_str("</svg>");
    s
}

/// A face-down domino.
pub fn back_svg(vertical: bool) -> String {
    let (w, h) = if vertical { (100, 200) } else { (200, 100) };
    format!(
        r##"<svg viewBox="0 0 {w} {h}" class="domino-svg" xmlns="http://www.w3.org/2000/svg"><rect x="2" y="2" width="{}" height="{}" rx="14" fill="#7f1d1d" stroke="#450a0a" stroke-width="3"/><rect x="14" y="14" width="{}" height="{}" rx="8" fill="none" stroke="#fca5a5" stroke-opacity="0.45" stroke-width="3"/></svg>"##,
        w - 4,
        h - 4,
        w - 28,
        h - 28
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_twelve_set() {
        let set = full_set(12);
        assert_eq!(set.len(), 91);
        assert_eq!(set_size(12), 91);
        assert_eq!(set.iter().filter(|t| t.is_double()).count(), 13);
        assert!(set.iter().enumerate().all(|(i, t)| t.id as usize == i));
        // Each number shows up on 13 tiles.
        for n in 0..=12 {
            assert_eq!(set.iter().filter(|t| t.has(n)).count(), 13);
        }
    }

    #[test]
    fn pip_counts() {
        for n in 0..=12 {
            assert_eq!(pip_spots(n).len(), n as usize);
        }
    }
}

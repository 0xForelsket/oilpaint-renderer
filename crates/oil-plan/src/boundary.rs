//! Outer boundaries of a binary mask, as ordered pixel chains (curve placement along a region's outline; v1 used
//! OpenCV's `findContours` with external contours only). Moore-neighbour tracing, 8-connected, one chain per
//! component, components in raster order of their first pixel.

/// The 8 neighbours clockwise from west (x right, y down).
const DIRS: [(i64, i64); 8] = [(-1, 0), (-1, -1), (0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1)];

/// Outer boundary chains of `inside(i, j)` over the box `x0..x1, y0..y1` (pixels outside the box are outside).
pub fn outer_contours(x0: usize, y0: usize, x1: usize, y1: usize, inside: impl Fn(usize, usize) -> bool) -> Vec<Vec<[f64; 2]>> {
    let (bw, bh) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let at = |i: i64, j: i64| i >= 0 && j >= 0 && (i as usize) < bw && (j as usize) < bh && inside(x0 + i as usize, y0 + j as usize);
    let mut seen = vec![false; bw * bh];
    let mut out = Vec::new();
    for j in 0..bh as i64 {
        for i in 0..bw as i64 {
            if !at(i, j) || seen[j as usize * bw + i as usize] {
                continue;
            }
            // mark the component (8-connected flood fill)
            let mut stack = vec![(i, j)];
            seen[j as usize * bw + i as usize] = true;
            while let Some((a, b)) = stack.pop() {
                for (dx, dy) in DIRS {
                    let (c, d) = (a + dx, b + dy);
                    if at(c, d) && !seen[d as usize * bw + c as usize] {
                        seen[d as usize * bw + c as usize] = true;
                        stack.push((c, d));
                    }
                }
            }
            // (i, j) is the first pixel of the component in raster order, so its west neighbour is outside
            out.push(trace(i, j, &at).into_iter().map(|(a, b)| [(x0 as i64 + a) as f64, (y0 as i64 + b) as f64]).collect());
        }
    }
    out
}

/// Moore-neighbour trace from a start pixel whose west neighbour is outside; stops when the start is re-entered in
/// the same direction (Jacob's criterion).
fn trace(si: i64, sj: i64, at: &impl Fn(i64, i64) -> bool) -> Vec<(i64, i64)> {
    let mut chain = vec![(si, sj)];
    let (mut p, mut back) = ((si, sj), 0usize); // `back`: direction index from p to the last outside pixel
    let mut first_move: Option<(i64, i64)> = None;
    for _ in 0..4 * 1024 * 1024 {
        let mut found = None;
        for k in 1..=8 {
            let d = (back + k) % 8;
            let q = (p.0 + DIRS[d].0, p.1 + DIRS[d].1);
            if at(q.0, q.1) {
                found = Some((q, d));
                break;
            }
        }
        let Some((q, d)) = found else { return chain }; // an isolated pixel
        if p == (si, sj) {
            match first_move {
                None => first_move = Some(q),
                Some(f) if f == q => {
                    chain.pop();
                    return chain;
                }
                _ => {}
            }
        }
        // the new backtrack: the neighbour checked just before q, seen from q
        let prev = (p.0 + DIRS[(d + 7) % 8].0, p.1 + DIRS[(d + 7) % 8].1);
        back = DIRS.iter().position(|&(dx, dy)| (q.0 + dx, q.1 + dy) == prev).unwrap_or(0);
        p = q;
        chain.push(p);
    }
    chain
}

#[cfg(test)]
mod tests {
    use super::outer_contours;

    #[test]
    fn square_and_two_components() {
        // a 4x3 block and a single pixel
        let inside = |i: usize, j: usize| (2..6).contains(&i) && (1..4).contains(&j) || (i, j) == (8, 8);
        let c = outer_contours(0, 0, 10, 10, inside);
        assert_eq!(c.len(), 2);
        // perimeter pixels of a 4x3 block: 2*4 + 2*1 = 10
        assert_eq!(c[0].len(), 10, "{:?}", c[0]);
        assert_eq!(c[0][0], [2.0, 1.0]);
        assert_eq!(c[1], vec![[8.0, 8.0]]);
        // a ring: only its outer boundary is traced
        let ring = |i: usize, j: usize| {
            let (x, y) = (i as i64 - 10, j as i64 - 10);
            let r2 = x * x + y * y;
            (25..=64).contains(&r2)
        };
        let c = outer_contours(0, 0, 21, 21, ring);
        assert_eq!(c.len(), 1);
        assert!(c[0].iter().all(|p| ((p[0] - 10.0) * (p[0] - 10.0) + (p[1] - 10.0) * (p[1] - 10.0)).sqrt() > 6.0));
    }
}

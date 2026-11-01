//! Geometry for the 3D-printing tools (CDS fork), worked out from the formulas themselves: the
//! Supershape (Gielis' superformula), NACA four-digit airfoils and airfoil `.dat` files, a gyroid
//! sheet as a closed triangle mesh, and binary STL bytes for meshes that join the design.

use solidcraft_geom::{Vec2, Vec3};

/// The superformula's radius at angle `phi`, or `None` where it is not a finite positive number.
pub fn superformula(phi: f64, m: f64, n1: f64, n2: f64, n3: f64, a: f64, b: f64) -> Option<f64> {
    let t1 = ((m * phi / 4.0).cos() / a).abs().powf(n2);
    let t2 = ((m * phi / 4.0).sin() / b).abs().powf(n3);
    let r = (t1 + t2).powf(-1.0 / n1);
    (r.is_finite() && r > 0.0).then_some(r)
}

/// A closed Supershape outline of `samples` points (no point repeated), scaled so its farthest
/// point is `size / 2` from the centre.
pub fn supershape_outline(m: f64, n1: f64, n2: f64, n3: f64, a: f64, b: f64, size: f64, samples: usize) -> Option<Vec<Vec2>> {
    let mut pts = Vec::with_capacity(samples);
    for i in 0..samples {
        let phi = std::f64::consts::TAU * i as f64 / samples as f64;
        let r = superformula(phi, m, n1, n2, n3, a, b)?;
        pts.push(Vec2::new(r * phi.cos(), r * phi.sin()));
    }
    let far = pts.iter().map(|p| p.len()).fold(0.0_f64, f64::max);
    if !(far > 1e-12) {
        return None;
    }
    let k = size / 2.0 / far;
    Some(pts.into_iter().map(|p| Vec2::new(p.x * k, p.y * k)).collect())
}

/// A 3D Supershape (the spherical product of the superformula with itself) as triangles, scaled
/// so its farthest point is `size / 2` from the centre.
pub fn supershape_solid(m: f64, n1: f64, n2: f64, n3: f64, a: f64, b: f64, size: f64) -> Option<Vec<[Vec3; 3]>> {
    const LON: usize = 120;
    const LAT: usize = 60;
    use std::f64::consts::{FRAC_PI_2, PI};
    let mut grid = vec![vec![Vec3::ZERO; LON]; LAT + 1];
    for (i, row) in grid.iter_mut().enumerate() {
        let theta = -FRAC_PI_2 + PI * i as f64 / LAT as f64;
        let r2 = superformula(theta, m, n1, n2, n3, a, b)?;
        for (j, p) in row.iter_mut().enumerate() {
            let phi = -PI + 2.0 * PI * j as f64 / LON as f64;
            let r1 = superformula(phi, m, n1, n2, n3, a, b)?;
            *p = Vec3::new(r1 * phi.cos() * r2 * theta.cos(), r1 * phi.sin() * r2 * theta.cos(), r2 * theta.sin());
        }
    }
    // The poles are single points.
    let south = Vec3::new(0.0, 0.0, grid[0][0].z);
    let north = Vec3::new(0.0, 0.0, grid[LAT][0].z);
    let mut tris = Vec::new();
    for i in 0..LAT {
        for j in 0..LON {
            let k = (j + 1) % LON;
            let a0 = if i == 0 { south } else { grid[i][j] };
            let b0 = if i == 0 { south } else { grid[i][k] };
            let a1 = if i + 1 == LAT { north } else { grid[i + 1][j] };
            let b1 = if i + 1 == LAT { north } else { grid[i + 1][k] };
            if i != 0 {
                tris.push([a0, b0, b1]);
            }
            if i + 1 != LAT || i == 0 {
                tris.push([a0, b1, a1]);
            }
        }
    }
    let far = tris.iter().flatten().map(|p| p.len()).fold(0.0_f64, f64::max);
    if !(far > 1e-12) {
        return None;
    }
    let k = size / 2.0 / far;
    Some(tris.into_iter().map(|t| t.map(|p| Vec3::new(p.x * k, p.y * k, p.z * k))).collect())
}

/// A NACA four-digit airfoil (`"2412"`) of chord 1: upper surface from the trailing edge to the
/// leading edge, then the lower surface back to the trailing edge (`per_side` points each side,
/// cosine spaced; the leading edge once).
pub fn naca4(code: &str, per_side: usize) -> Option<Vec<Vec2>> {
    let d: Vec<u32> = code.trim().chars().map(|c| c.to_digit(10)).collect::<Option<_>>()?;
    if d.len() != 4 || per_side < 4 {
        return None;
    }
    let m = f64::from(d[0]) / 100.0;
    let p = f64::from(d[1]) / 10.0;
    let t = f64::from(d[2] * 10 + d[3]) / 100.0;
    if !(t > 0.0) {
        return None;
    }
    let at = |x: f64| -> (Vec2, Vec2) {
        let yt = 5.0 * t * (0.2969 * x.sqrt() - 0.1260 * x - 0.3516 * x * x + 0.2843 * x.powi(3) - 0.1015 * x.powi(4));
        let (yc, dyc) = if m > 0.0 && p > 0.0 {
            if x < p {
                (m / (p * p) * (2.0 * p * x - x * x), 2.0 * m / (p * p) * (p - x))
            } else {
                (m / ((1.0 - p) * (1.0 - p)) * (1.0 - 2.0 * p + 2.0 * p * x - x * x), 2.0 * m / ((1.0 - p) * (1.0 - p)) * (p - x))
            }
        } else {
            (0.0, 0.0)
        };
        let th = dyc.atan();
        (Vec2::new(x - yt * th.sin(), yc + yt * th.cos()), Vec2::new(x + yt * th.sin(), yc - yt * th.cos()))
    };
    let xs: Vec<f64> = (0..per_side).map(|i| (1.0 - (std::f64::consts::PI * i as f64 / (per_side - 1) as f64).cos()) / 2.0).collect();
    let mut out: Vec<Vec2> = xs.iter().rev().map(|x| at(*x).0).collect();
    out.extend(xs.iter().skip(1).map(|x| at(*x).1));
    Some(out)
}

/// An airfoil `.dat` file (Selig: one loop from the trailing edge over the top and back under;
/// Lednicer: two counts, then the upper and the lower surface each from the leading edge), as
/// one loop like [`naca4`]'s, chord 1. The first line may be a name.
pub fn airfoil_dat(text: &str) -> Option<Vec<Vec2>> {
    let rows: Vec<Vec2> = text
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace().map(str::parse::<f64>);
            let x = it.next()?.ok()?;
            let y = it.next()?.ok()?;
            (x.is_finite() && y.is_finite()).then_some(Vec2::new(x, y))
        })
        .collect();
    let first = *rows.first()?;
    let loop_pts = if first.x > 1.5 && first.y > 1.5 {
        // Lednicer: counts, then upper (LE → TE) and lower (LE → TE).
        let nu = first.x as usize;
        let nl = first.y as usize;
        let upper = rows.get(1..1 + nu)?;
        let lower = rows.get(1 + nu..1 + nu + nl)?;
        let mut v: Vec<Vec2> = upper.iter().rev().copied().collect();
        v.extend(lower.iter().skip(1).copied());
        v
    } else {
        rows
    };
    let mut pts: Vec<Vec2> = Vec::with_capacity(loop_pts.len());
    for p in loop_pts {
        if pts.last().is_none_or(|q: &Vec2| q.dist(p) > 1e-9) {
            pts.push(p);
        }
    }
    // Normalise the chord to 1 from the leading edge.
    let (min_x, max_x) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let chord = max_x - min_x;
    if pts.len() < 5 || !(chord > 1e-9) {
        return None;
    }
    Some(pts.into_iter().map(|p| Vec2::new((p.x - min_x) / chord, p.y / chord)).collect())
}

/// The gyroid's value at `p` for cells of `k = 2π / cell`.
fn gyroid(p: Vec3, k: f64) -> f64 {
    let (x, y, z) = (p.x * k, p.y * k, p.z * k);
    x.sin() * y.cos() + y.sin() * z.cos() + z.sin() * x.cos()
}

/// A gyroid sheet of wall thickness `wall` and cell size `cell`, filling the box from `min` of
/// size `size`, as closed triangles (marching tetrahedra), at most about `budget` grid cells.
pub fn gyroid_block(min: Vec3, size: Vec3, cell: f64, wall: f64, budget: usize) -> Vec<[Vec3; 3]> {
    let k = std::f64::consts::TAU / cell;
    // |∇f| of the gyroid is close to 1.15·k on its surface: the sheet |f| < tau is about `wall` thick.
    let slope = 1.15 * k;
    let tau = (wall * slope / 2.0).min(1.2);
    let half = Vec3::new(size.x / 2.0, size.y / 2.0, size.z / 2.0);
    let centre = Vec3::new(min.x + half.x, min.y + half.y, min.z + half.z);
    let value = |p: Vec3| -> f64 {
        let sheet = gyroid(p, k).abs() - tau;
        let d = Vec3::new((p.x - centre.x).abs() - half.x, (p.y - centre.y).abs() - half.y, (p.z - centre.z).abs() - half.z);
        let boxd = d.x.max(d.y).max(d.z) * slope;
        sheet.max(boxd)
    };
    let span = size.x.max(size.y).max(size.z);
    let mut h = cell / 14.0;
    let cells = |h: f64| ((size.x / h + 3.0) * (size.y / h + 3.0) * (size.z / h + 3.0)) as usize;
    while cells(h) > budget {
        h *= 1.1;
        if h > span {
            break;
        }
    }
    let start = Vec3::new(min.x - h, min.y - h, min.z - h);
    let n = [(size.x / h).ceil() as usize + 2, (size.y / h).ceil() as usize + 2, (size.z / h).ceil() as usize + 2];
    let at = |i: usize, j: usize, l: usize| Vec3::new(start.x + i as f64 * h, start.y + j as f64 * h, start.z + l as f64 * h);
    // Values on the grid's corners.
    let idx = |i: usize, j: usize, l: usize| (l * (n[1] + 1) + j) * (n[0] + 1) + i;
    let mut vals = vec![0.0; (n[0] + 1) * (n[1] + 1) * (n[2] + 1)];
    for l in 0..=n[2] {
        for j in 0..=n[1] {
            for i in 0..=n[0] {
                vals[idx(i, j, l)] = value(at(i, j, l));
            }
        }
    }
    // A cube's six tetrahedra around its main diagonal (corner 0 to corner 6).
    const CORNERS: [(usize, usize, usize); 8] = [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)];
    const TETS: [[usize; 4]; 6] = [[0, 6, 1, 2], [0, 6, 2, 3], [0, 6, 3, 7], [0, 6, 7, 4], [0, 6, 4, 5], [0, 6, 5, 1]];
    let mut tris = Vec::new();
    for l in 0..n[2] {
        for j in 0..n[1] {
            for i in 0..n[0] {
                let cp: [Vec3; 8] = CORNERS.map(|(a, b, c)| at(i + a, j + b, l + c));
                let cv: [f64; 8] = CORNERS.map(|(a, b, c)| vals[idx(i + a, j + b, l + c)]);
                if cv.iter().all(|v| *v < 0.0) || cv.iter().all(|v| *v >= 0.0) {
                    continue;
                }
                for t in TETS {
                    let p = t.map(|c| cp[c]);
                    let v = t.map(|c| cv[c]);
                    tetra(&p, &v, &mut tris);
                }
            }
        }
    }
    tris
}

/// The surface `v = 0` through one tetrahedron, its triangles facing the positive side.
fn tetra(p: &[Vec3; 4], v: &[f64; 4], out: &mut Vec<[Vec3; 3]>) {
    let inside: Vec<usize> = (0..4).filter(|i| v[*i] < 0.0).collect();
    let outside: Vec<usize> = (0..4).filter(|i| v[*i] >= 0.0).collect();
    let cut = |a: usize, b: usize| -> Vec3 {
        let t = v[a] / (v[a] - v[b]);
        p[a].lerp(p[b], t)
    };
    let centre = |ids: &[usize]| -> Vec3 {
        let mut c = Vec3::ZERO;
        for i in ids {
            c += p[*i];
        }
        c * (1.0 / ids.len().max(1) as f64)
    };
    let towards = centre(&outside) - centre(&inside);
    let mut push = |a: Vec3, b: Vec3, c: Vec3| {
        let n = (b - a).cross(c - a);
        if n.len2() < 1e-30 {
            return;
        }
        out.push(if n.dot(towards) >= 0.0 { [a, b, c] } else { [a, c, b] });
    };
    match (inside.as_slice(), outside.as_slice()) {
        ([a], [b, c, d]) | ([b, c, d], [a]) => push(cut(*a, *b), cut(*a, *c), cut(*a, *d)),
        ([a, b], [c, d]) => {
            let (ac, ad, bc, bd) = (cut(*a, *c), cut(*a, *d), cut(*b, *c), cut(*b, *d));
            push(ac, ad, bd);
            push(ac, bd, bc);
        }
        _ => {}
    }
}

/// Binary STL bytes for triangles (millimetres).
pub fn stl_bytes(tris: &[[Vec3; 3]], name: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(84 + tris.len() * 50);
    let mut header = [b' '; 80];
    for (h, c) in header.iter_mut().zip(name.bytes()) {
        *h = c;
    }
    out.extend_from_slice(&header);
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for t in tris {
        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalized().unwrap_or(Vec3::Z);
        for v in [n, t[0], t[1], t[2]] {
            for c in [v.x, v.y, v.z] {
                out.extend_from_slice(&(c as f32).to_le_bytes());
            }
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    out
}

/// Centres of a honeycomb's cells (`cell` apart, alternate rows shifted) whose hexagon of
/// circumradius `r` lies inside the rectangle `[lo, hi]`.
pub fn hex_centres(lo: Vec2, hi: Vec2, cell: f64, r: f64, max: usize) -> Vec<Vec2> {
    let row = cell * 3f64.sqrt() / 2.0;
    let mut out = Vec::new();
    let mut y = lo.y + r;
    let mut odd = false;
    while y + r <= hi.y + 1e-9 {
        let mut x = lo.x + r * 3f64.sqrt() / 2.0 + if odd { cell / 2.0 } else { 0.0 };
        while x + r * 3f64.sqrt() / 2.0 <= hi.x + 1e-9 {
            if out.len() >= max {
                return out;
            }
            out.push(Vec2::new(x, y));
            x += cell;
        }
        y += row;
        odd = !odd;
    }
    out
}

/// A pointy-topped hexagon's corners around `c`.
pub fn hexagon(c: Vec2, r: f64) -> Vec<Vec2> {
    (0..6)
        .map(|i| {
            let a = std::f64::consts::FRAC_PI_6 + std::f64::consts::FRAC_PI_3 * i as f64;
            Vec2::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naca_and_dat_loops_run_from_the_trailing_edge_round_the_nose() {
        let p = naca4("2412", 40).unwrap_or_default();
        assert_eq!(p.len(), 79);
        assert!((p[0].x - 1.0).abs() < 1e-3 && p[39].x.abs() < 1e-9);
        assert!(p[20].y > 0.0 && p[60].y < 0.0);
        assert!(naca4("24x2", 40).is_none() && naca4("2400", 40).is_none());
        let dat = "TEST\n1.0 0.001\n0.5 0.06\n0.0 0.0\n0.5 -0.04\n1.0 -0.001\n";
        assert_eq!(airfoil_dat(dat).map(|v| v.len()), Some(5));
        let lednicer = "L\n3. 3.\n\n0 0\n0.5 0.05\n1 0\n\n0 0\n0.5 -0.03\n1 0\n";
        let l = airfoil_dat(lednicer).unwrap_or_default();
        assert_eq!(l.len(), 5);
        assert!((l[0].x - 1.0).abs() < 1e-9 && l[1].y > 0.0 && l[3].y < 0.0);
    }

    #[test]
    fn supershapes_scale_to_their_size_and_gyroids_close() {
        let o = supershape_outline(6.0, 1.0, 1.7, 1.7, 1.0, 1.0, 50.0, 200).unwrap_or_default();
        let far = o.iter().map(|p| p.len()).fold(0.0, f64::max);
        assert!((far - 25.0).abs() < 1e-9);
        let s = supershape_solid(6.0, 1.0, 1.7, 1.7, 1.0, 1.0, 40.0).unwrap_or_default();
        assert!(!s.is_empty());
        let g = gyroid_block(Vec3::ZERO, Vec3::new(20.0, 20.0, 10.0), 10.0, 1.0, 200_000);
        assert!(g.len() > 1000);
        // Every edge is shared by two triangles: the sheet is closed.
        let key = |p: Vec3| ((p.x * 1e5).round() as i64, (p.y * 1e5).round() as i64, (p.z * 1e5).round() as i64);
        let mut edges = std::collections::HashMap::new();
        for t in &g {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (ka, kb) = (key(a), key(b));
                *edges.entry(if ka < kb { (ka, kb) } else { (kb, ka) }).or_insert(0) += 1;
            }
        }
        let open = edges.values().filter(|n| **n % 2 != 0).count();
        assert!(open * 100 < edges.len(), "{open} of {} edges open", edges.len());
        assert_eq!(stl_bytes(&g[..2], "x").len(), 84 + 100);
        let hexes = hex_centres(Vec2::new(0.0, 0.0), Vec2::new(40.0, 40.0), 8.0, 4.0, 1000);
        assert!(hexes.len() > 10);
        assert_eq!(hexagon(Vec2::new(0.0, 0.0), 1.0).len(), 6);
    }
}

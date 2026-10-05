//! Deterministic gradient noise on f64 coordinates (large world coordinates stay precise).

use super::rng::hash2;

const GRAD: [(f64, f64); 16] = [
    (1.0, 0.0),
    (0.923_879_532_511_286_7, 0.382_683_432_365_089_8),
    (0.707_106_781_186_547_6, 0.707_106_781_186_547_6),
    (0.382_683_432_365_089_8, 0.923_879_532_511_286_7),
    (0.0, 1.0),
    (-0.382_683_432_365_089_8, 0.923_879_532_511_286_7),
    (-0.707_106_781_186_547_6, 0.707_106_781_186_547_6),
    (-0.923_879_532_511_286_7, 0.382_683_432_365_089_8),
    (-1.0, 0.0),
    (-0.923_879_532_511_286_7, -0.382_683_432_365_089_8),
    (-0.707_106_781_186_547_6, -0.707_106_781_186_547_6),
    (-0.382_683_432_365_089_8, -0.923_879_532_511_286_7),
    (0.0, -1.0),
    (0.382_683_432_365_089_8, -0.923_879_532_511_286_7),
    (0.707_106_781_186_547_6, -0.707_106_781_186_547_6),
    (0.923_879_532_511_286_7, -0.382_683_432_365_089_8),
];

const SCALE: f64 = 1.414;

#[inline]
fn grad(seed: u64, x: i64, y: i64) -> (f64, f64) {
    GRAD[(hash2(seed, x, y) & 15) as usize]
}

#[inline]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn dfade(t: f64) -> f64 {
    30.0 * t * t * (t - 1.0) * (t - 1.0)
}

/// Gradient noise in roughly [-1, 1]. The value half of `gradient2_d` (same arithmetic, so
/// the same bits) without the derivative work.
pub fn gradient2(seed: u64, x: f64, y: f64) -> f64 {
    let xf = crate::core::floor(x);
    let yf = crate::core::floor(y);
    let fx = x - xf;
    let fy = y - yf;
    let (ix, iy) = (xf as i64, yf as i64);

    let g00 = grad(seed, ix, iy);
    let g10 = grad(seed, ix + 1, iy);
    let g01 = grad(seed, ix, iy + 1);
    let g11 = grad(seed, ix + 1, iy + 1);

    let n00 = g00.0 * fx + g00.1 * fy;
    let n10 = g10.0 * (fx - 1.0) + g10.1 * fy;
    let n01 = g01.0 * fx + g01.1 * (fy - 1.0);
    let n11 = g11.0 * (fx - 1.0) + g11.1 * (fy - 1.0);

    let u = fade(fx);
    let v = fade(fy);
    let k = n00 - n10 - n01 + n11;
    let value = n00 + u * (n10 - n00) + v * (n01 - n00) + u * v * k;
    value * SCALE
}

/// Gradient noise with analytic derivatives: (value, d/dx, d/dy).
pub fn gradient2_d(seed: u64, x: f64, y: f64) -> (f64, f64, f64) {
    let xf = crate::core::floor(x);
    let yf = crate::core::floor(y);
    let fx = x - xf;
    let fy = y - yf;
    let (ix, iy) = (xf as i64, yf as i64);

    let g00 = grad(seed, ix, iy);
    let g10 = grad(seed, ix + 1, iy);
    let g01 = grad(seed, ix, iy + 1);
    let g11 = grad(seed, ix + 1, iy + 1);

    let n00 = g00.0 * fx + g00.1 * fy;
    let n10 = g10.0 * (fx - 1.0) + g10.1 * fy;
    let n01 = g01.0 * fx + g01.1 * (fy - 1.0);
    let n11 = g11.0 * (fx - 1.0) + g11.1 * (fy - 1.0);

    let u = fade(fx);
    let v = fade(fy);
    let du = dfade(fx);
    let dv = dfade(fy);

    let k = n00 - n10 - n01 + n11;
    let value = n00 + u * (n10 - n00) + v * (n01 - n00) + u * v * k;
    let dx = g00.0
        + u * (g10.0 - g00.0)
        + v * (g01.0 - g00.0)
        + u * v * (g00.0 - g10.0 - g01.0 + g11.0)
        + du * ((n10 - n00) + v * k);
    let dy = g00.1
        + u * (g10.1 - g00.1)
        + v * (g01.1 - g00.1)
        + u * v * (g00.1 - g10.1 - g01.1 + g11.1)
        + dv * ((n01 - n00) + u * k);
    (value * SCALE, dx * SCALE, dy * SCALE)
}

/// Fractal Brownian motion, normalized to roughly [-1, 1].
pub fn fbm(seed: u64, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..octaves {
        let s = seed.wrapping_add((o as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        sum += amp * gradient2(s, x * freq, y * freq);
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }
    sum / norm
}

/// Ridged multifractal in roughly [0, 1]: sharp crests, for mountain ranges.
pub fn ridged(seed: u64, x: f64, y: f64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut freq, mut norm, mut weight) = (0.0, 1.0, 1.0, 0.0, 1.0);
    for o in 0..octaves {
        let s = seed.wrapping_add((o as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut r = 1.0 - crate::core::fabs(gradient2(s, x * freq, y * freq));
        r *= r;
        sum += r * amp * weight;
        norm += amp;
        weight = (r * 2.0).clamp(0.0, 1.0);
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

#[inline]
pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

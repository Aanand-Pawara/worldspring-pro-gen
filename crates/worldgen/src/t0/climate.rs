//! Coarse, deterministic climate simulation.
//!
//! The model is intentionally not full fluid dynamics. It solves the fields that matter for a
//! procedural planet: seasonal solar forcing, lapse-rate cooling, maritime moderation,
//! continentality, circulation, 2-D moisture advection, evaporation, orographic lift, soil
//! water, and snowpack. Noise is only a small post-physical perturbation.

use crate::World;
use crate::core::noise::{fbm, smoothstep};
pub const MONTHS: usize = 12;

use crate::world::{ClimateParams, Wind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ClimateClass {
    TropicalRainforest = 0,
    TropicalSeasonal = 1,
    Savanna = 2,
    HotDesert = 3,
    ColdDesert = 4,
    SemiArid = 5,
    Mediterranean = 6,
    TemperateOceanic = 7,
    TemperateContinental = 8,
    TemperateRainforest = 9,
    Boreal = 10,
    Subarctic = 11,
    Tundra = 12,
    Alpine = 13,
    Polar = 14,
}

#[derive(Clone, Debug, Default)]
pub struct Climate {
    pub temp: Vec<f32>,
    pub precip: Vec<f32>,
    pub humidity: Vec<f32>,
    pub evap: Vec<f32>,
    pub soil_moisture: Vec<f32>,
    pub snow: Vec<f32>,
    pub snowmelt: Vec<f32>,
    pub continentality: Vec<f32>,
    pub orographic: Vec<f32>,
    pub wind_u: Vec<f32>,
    pub wind_v: Vec<f32>,
    pub temp_range: Vec<f32>,
    pub summer_precip: Vec<f32>,
    pub winter_precip: Vec<f32>,
    pub snowfall: Vec<f32>,
    pub growing_season: Vec<f32>,
    pub class: Vec<u8>,
    pub monthly_temp: Vec<f32>,
    pub monthly_precip: Vec<f32>,
    pub monthly_humidity: Vec<f32>,
    pub monthly_evap: Vec<f32>,
    pub monthly_soil: Vec<f32>,
    pub monthly_snow: Vec<f32>,
}

pub fn latitude(world: &World, j: usize, h: usize) -> f64 {
    let p = world.params();
    if h <= 1 { return 0.5 * (p.lat_top + p.lat_bottom); }
    p.lat_top + (p.lat_bottom - p.lat_top) * j as f64 / (h - 1) as f64
}

pub fn sea_level_temp(lat: f64) -> f64 {
    let s = libm::sin(lat.to_radians()).abs();
    28.0 - 36.0 * libm::pow(s, 1.45)
}

pub fn build(world: &World, w: usize, h: usize, cell_ft: f64, height: &[f64], land: &[bool]) -> Climate {
    let p = world.params();
    let cfg: ClimateParams = world.params().climate;
    let n = w * h;
    let sea = p.sea_level_ft;
    let cell_mi = cell_ft / 5280.0;

    let mut smooth = height.to_vec();
    for _ in 0..5 { smooth = box_blur(w, h, &smooth, land); }

    let ocean: Vec<bool> = land.iter().map(|&v| !v).collect();
    let coast_dist = distance_to(w, h, &ocean);
    let mut continentality = vec![0.0f32; n];
    for k in 0..n {
        if land[k] {
            let d_mi = coast_dist[k] * cell_mi;
            continentality[k] = (cfg.continentality_strength * (1.0 - libm::exp(-d_mi / 650.0))).clamp(0.0, 1.0) as f32;
        }
    }

    let mut temp = vec![0.0f32; n];
    let mut precip = vec![0.0f32; n];
    let mut humidity = vec![0.35f32; n];
    let mut evap = vec![0.0f32; n];
    let mut soil = vec![0.35f32; n];
    let mut snow = vec![0.0f32; n];
    let mut snowmelt = vec![0.0f32; n];
    let mut orographic = vec![0.0f32; n];
    let mut wind_u = vec![0.0f32; n];
    let mut wind_v = vec![0.0f32; n];

    let mut monthly_temp = vec![0.0f32; n * MONTHS];
    let mut monthly_precip = vec![0.0f32; n * MONTHS];
    let mut monthly_humidity = vec![0.0f32; n * MONTHS];
    let mut monthly_evap = vec![0.0f32; n * MONTHS];
    let mut monthly_soil = vec![0.0f32; n * MONTHS];
    let mut monthly_snow = vec![0.0f32; n * MONTHS];
    let mut summer_precip = vec![0.0f32; n];
    let mut winter_precip = vec![0.0f32; n];
    let mut snowfall = vec![0.0f32; n];
    let mut growing = vec![0.0f32; n];
    let mut temp_min = vec![f32::INFINITY; n];
    let mut temp_max = vec![f32::NEG_INFINITY; n];

    let seed = world.stream("t0.climate.perturb");
    let two_pi = 2.0 * std::f64::consts::PI;

    for month in 0..MONTHS {
        let phase = (month as f64 + 0.5) / 12.0;
        let decl = cfg.axial_tilt_deg * libm::sin(two_pi * (phase - 0.5));
        let mut base_temp = vec![0.0f64; n];

        for j in 0..h {
            let lat = latitude(world, j, h);
            let alat = lat.abs();
            let lat_norm = libm::sin(alat.to_radians()).abs().clamp(0.0, 1.0);
            let annual = cfg.equatorial_temp_c
                + (cfg.polar_temp_c - cfg.equatorial_temp_c) * libm::pow(lat_norm, 1.45);
            let seasonal = 13.0 * cfg.seasonality_strength
                * libm::cos((lat - decl).to_radians()) * (1.0 - 0.55 * lat_norm);

            for i in 0..w {
                let k = j * w + i;
                let above_km = if land[k] { (height[k] - sea).max(0.0) * 0.0003048 } else { 0.0 };
                let ocean_temp = cfg.equatorial_temp_c
                    + (cfg.polar_temp_c - cfg.equatorial_temp_c) * libm::pow(lat_norm, 1.25)
                    + 4.0 * libm::cos((lat - decl).to_radians()) * cfg.seasonality_strength;
                let continental_swing = 10.0 * continentality[k] as f64 * cfg.seasonality_strength
                    * libm::cos((lat - decl).to_radians());
                let ocean_share = cfg.ocean_temp_influence * continentality[k] as f64;
                let mut t = annual + seasonal + continental_swing
                    - cfg.lapse_rate_c_per_km * above_km;
                if land[k] {
                    t = t * (1.0 - ocean_share) + ocean_temp * ocean_share;
                } else {
                    t = ocean_temp;
                }
                let solar = solar_factor(lat, decl) * (cfg.solar_constant / 1361.0);
                let perturb = 0.65 * fbm(seed, i as f64 / 80.0, j as f64 / 80.0, 3, 2.0, 0.5);
                base_temp[k] = t + p.temp_offset_c + 1.5 * (solar - 0.5) + perturb;
            }
        }

        let mut wu = vec![0.0f64; n];
        let mut wv = vec![0.0f64; n];
        for j in 0..h {
            let lat = latitude(world, j, h);
            let alat = lat.abs();
            let zonal = match p.wind {
                Wind::FromWest => 1.0,
                Wind::FromEast => -1.0,
                Wind::Belts => {
                    let mid = smoothstep(25.0, 35.0, alat);
                    let polar = smoothstep(55.0, 65.0, alat);
                    2.0 * mid - 1.0 - 2.0 * polar
                }
            };
            // Convergence toward the seasonally migrating ITCZ.
            let meridional = ((decl - lat) / 18.0).clamp(-1.0, 1.0) * 0.22;
            for i in 0..w {
                let k = j * w + i;
                wu[k] = zonal * cfg.wind_strength;
                wv[k] = meridional * cfg.wind_strength;
                wind_u[k] += wu[k] as f32 / MONTHS as f32;
                wind_v[k] += wv[k] as f32 / MONTHS as f32;
            }
        }

        let mut local_evap = vec![0.0f64; n];
        for k in 0..n {
            let t = base_temp[k];
            let warm = smoothstep(-5.0, 32.0, t);
            let solar = solar_factor(latitude(world, k / w, h), decl);
            let wet = soil[k] as f64;
            let humidity_deficit = (1.0 - humidity[k] as f64).clamp(0.05, 1.0);
            let source = if !land[k] {
                3.2 * warm * (0.35 + 0.65 * solar)
            } else {
                1.4 * warm * wet * (0.3 + 0.7 * solar)
            };
            local_evap[k] = source * cfg.evaporation_strength * humidity_deficit;
        }

        let mut next_h = humidity.iter().map(|&v| v as f64).collect::<Vec<_>>();
        for _ in 0..3 {
            for j in 0..h {
                for i in 0..w {
                    let k = j * w + i;
                    let speed = libm::sqrt(wu[k] * wu[k] + wv[k] * wv[k]).max(0.2);
                    let step = (cfg.moisture_transport_strength * (0.7 + 0.35 * speed)).clamp(0.5, 2.5);
                    let upstream = sample_bilinear_f32(&humidity, w, h, i as f64 - wu[k] * step, j as f64 - wv[k] * step);
                    let source = if !land[k] { 1.0 } else { local_evap[k] * 0.17 };
                    next_h[k] = (0.62 * upstream as f64 + 0.38 * (humidity[k] as f64 + source)).clamp(0.0, 1.0);
                }
            }
            humidity.iter_mut().zip(&next_h).for_each(|(a, &b)| *a = b as f32);
        }

        let mut rain = vec![0.0f64; n];
        for j in 0..h {
            for i in 0..w {
                let k = j * w + i;
                let fx = (wu[k] * 2.0).clamp(-2.0, 2.0);
                let fy = (wv[k] * 2.0).clamp(-2.0, 2.0);
                let ahead = sample_bilinear(&smooth, w, h, i as f64 + fx, j as f64 + fy);
                let rise_km = ((ahead - smooth[k]).max(0.0) * 0.0003048) / 2.0;
                let lift = (rise_km * (0.65 + 0.35 * libm::sqrt(wu[k] * wu[k] + wv[k] * wv[k]))).clamp(0.0, 1.0);
                let lat = latitude(world, j, h);
                let convergence = libm::exp(-((lat - decl) / 16.0) * ((lat - decl) / 16.0));
                let saturation = (humidity[k] as f64 * (0.45 + 0.85 * convergence)).clamp(0.0, 1.5);
                let oro = lift * saturation * cfg.orographic_strength;
                orographic[k] += oro as f32 / MONTHS as f32;
                let base_rain = (0.018 + 0.07 * convergence) * saturation;
                rain[k] = ((base_rain + oro * (0.42 + 0.58 * cfg.rain_shadow_strength))
                    * 145.0 * (0.75 + 0.25 * solar_factor(lat, decl)) * p.moisture).max(0.0);
            }
        }

        for k in 0..n {
            let t = base_temp[k];
            let r = rain[k];
            let e = local_evap[k];
            let deficit = (r - e).max(0.0);
            let old_snow = snow[k] as f64;
            let snow_in = if t <= 1.5 { r } else { 0.0 };
            let melt = if t > 0.0 { (t * 0.10).min(1.0) * old_snow } else { 0.0 };
            if land[k] {
                soil[k] = (soil[k] as f64 + deficit * 0.010 - e * 0.004 - soil[k] as f64 * 0.03).clamp(0.0, 1.0) as f32;
            } else {
                soil[k] = 1.0;
            }
            snow[k] = (old_snow + snow_in * 0.012 - melt).max(0.0) as f32;
            snowmelt[k] += melt as f32;
            temp[k] += t as f32 / MONTHS as f32;
            precip[k] += r as f32;
            evap[k] += e as f32;
            let mi = month * n + k;
            monthly_temp[mi] = t as f32;
            monthly_precip[mi] = r as f32;
            monthly_humidity[mi] = humidity[k];
            monthly_evap[mi] = e as f32;
            monthly_soil[mi] = soil[k];
            monthly_snow[mi] = snow[k];
            temp_min[k] = temp_min[k].min(t as f32);
            temp_max[k] = temp_max[k].max(t as f32);
            if month >= 4 && month <= 9 { summer_precip[k] += r as f32; }
            else { winter_precip[k] += r as f32; }
            snowfall[k] += snow_in as f32;
            if t > 5.0 { growing[k] += 1.0; }
        }
    }

    let mut temp_range = vec![0.0f32; n];
    let mut class = vec![ClimateClass::TemperateContinental as u8; n];
    for k in 0..n {
        temp[k] = temp[k].clamp(-60.0, 60.0);
        precip[k] = precip[k].max(0.0);
        evap[k] = evap[k].max(0.0);
        humidity[k] = humidity[k].clamp(0.0, 1.0);
        temp_range[k] = (temp_max[k] - temp_min[k]).max(0.0);
        class[k] = classify(temp[k] as f64, precip[k] as f64, temp_range[k] as f64,
            summer_precip[k] as f64, winter_precip[k] as f64, snowfall[k] as f64,
            smooth_elevation(k, w, h, &smooth, sea), cfg.lapse_rate_c_per_km);
    }

    Climate {
        temp, precip, humidity, evap, soil_moisture: soil, snow, snowmelt, continentality,
        orographic, wind_u, wind_v, temp_range, summer_precip, winter_precip, snowfall,
        growing_season: growing, class, monthly_temp, monthly_precip, monthly_humidity,
        monthly_evap, monthly_soil, monthly_snow,
    }
}

fn classify(t: f64, p: f64, range: f64, summer: f64, winter: f64, snow: f64, elev: f64, _lapse: f64) -> u8 {
    if t <= -5.0 { return ClimateClass::Polar as u8; }
    if elev > 4500.0 && t < 8.0 { return ClimateClass::Alpine as u8; }
    if t < 2.0 { return if p < 300.0 { ClimateClass::ColdDesert as u8 } else { ClimateClass::Tundra as u8 }; }
    if t < 7.0 { return if p < 400.0 { ClimateClass::ColdDesert as u8 } else if snow > 500.0 { ClimateClass::Boreal as u8 } else { ClimateClass::Subarctic as u8 }; }
    if t > 22.0 {
        if p > 1800.0 { return ClimateClass::TropicalRainforest as u8; }
        if summer > winter * 1.55 && p > 650.0 { return ClimateClass::TropicalSeasonal as u8; }
        if p < 220.0 { return ClimateClass::HotDesert as u8; }
        if p < 650.0 { return ClimateClass::Savanna as u8; }
        return ClimateClass::TropicalSeasonal as u8;
    }
    if p < 180.0 { return ClimateClass::HotDesert as u8; }
    if p < 420.0 { return ClimateClass::SemiArid as u8; }
    if t >= 8.0 && t <= 20.0 && summer < winter * 0.62 && p > 450.0 { return ClimateClass::Mediterranean as u8; }
    if t >= 8.0 && t <= 18.0 && p > 1700.0 { return ClimateClass::TemperateRainforest as u8; }
    if t >= 8.0 && t <= 18.0 && range < 14.0 { return ClimateClass::TemperateOceanic as u8; }
    if t >= 4.0 && t < 12.0 && snow > 600.0 { return ClimateClass::Boreal as u8; }
    ClimateClass::TemperateContinental as u8
}

fn smooth_elevation(k: usize, _w: usize, _h: usize, smooth: &[f64], sea: f64) -> f64 {
    if k >= smooth.len() { return 0.0; }
    ((smooth[k] - sea).max(0.0) * 0.3048).max(0.0)
}

fn solar_factor(lat: f64, decl: f64) -> f64 {
    libm::cos((lat - decl).to_radians()).max(0.0)
}

fn sample_bilinear(v: &[f64], w: usize, h: usize, x: f64, y: f64) -> f64 {
    let x = x.clamp(0.0, (w - 1) as f64);
    let y = y.clamp(0.0, (h - 1) as f64);
    let x0 = libm::floor(x) as usize;
    let y0 = libm::floor(y) as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;
    let a = v[y0 * w + x0] * (1.0 - fx) + v[y0 * w + x1] * fx;
    let b = v[y1 * w + x0] * (1.0 - fx) + v[y1 * w + x1] * fx;
    a * (1.0 - fy) + b * fy
}

fn sample_bilinear_f32(v: &[f32], w: usize, h: usize, x: f64, y: f64) -> f32 {
    let x = x.clamp(0.0, (w - 1) as f64);
    let y = y.clamp(0.0, (h - 1) as f64);
    let x0 = libm::floor(x) as usize;
    let y0 = libm::floor(y) as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;
    let a = v[y0 * w + x0] as f64 * (1.0 - fx) + v[y0 * w + x1] as f64 * fx;
    let b = v[y1 * w + x0] as f64 * (1.0 - fx) + v[y1 * w + x1] as f64 * fx;
    (a * (1.0 - fy) + b * fy) as f32
}

fn box_blur(w: usize, h: usize, v: &[f64], land: &[bool]) -> Vec<f64> {
    let mut out = v.to_vec();
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            if !land[k] { continue; }
            let (mut sum, mut count) = (0.0, 0.0);
            for dj in -2i64..=2 {
                for di in -2i64..=2 {
                    let x = i as i64 + di;
                    let y = j as i64 + dj;
                    if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { continue; }
                    let kk = y as usize * w + x as usize;
                    if land[kk] { sum += v[kk]; count += 1.0; }
                }
            }
            if count > 0.0 { out[k] = sum / count; }
        }
    }
    out
}

pub fn distance_to(w: usize, h: usize, target: &[bool]) -> Vec<f64> {
    const D1: f64 = 1.0;
    const D2: f64 = std::f64::consts::SQRT_2;
    let big = (w + h) as f64 * 2.0;
    let mut d: Vec<f64> = target.iter().map(|&t| if t { 0.0 } else { big }).collect();
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            let mut x = d[k];
            if i > 0 { x = x.min(d[k - 1] + D1); }
            if j > 0 {
                x = x.min(d[k - w] + D1);
                if i > 0 { x = x.min(d[k - w - 1] + D2); }
                if i + 1 < w { x = x.min(d[k - w + 1] + D2); }
            }
            d[k] = x;
        }
    }
    for j in (0..h).rev() {
        for i in (0..w).rev() {
            let k = j * w + i;
            let mut x = d[k];
            if i + 1 < w { x = x.min(d[k + 1] + D1); }
            if j + 1 < h {
                x = x.min(d[k + w] + D1);
                if i + 1 < w { x = x.min(d[k + w + 1] + D2); }
                if i > 0 { x = x.min(d[k + w - 1] + D2); }
            }
            d[k] = x;
        }
    }
    d
}

pub fn validate(w: usize, h: usize, land: &[bool], height: &[f64], clim: &Climate) -> Vec<String> {
    let n = w * h;
    let mut errors = Vec::new();
    if clim.temp.len() != n || clim.precip.len() != n { errors.push("annual climate field size mismatch".into()); return errors; }
    for k in 0..n {
        if !clim.temp[k].is_finite() || !clim.precip[k].is_finite() || clim.precip[k] < 0.0 {
            errors.push(format!("invalid temperature/precipitation at {k}"));
            break;
        }
        if clim.humidity[k] < -0.001 || clim.humidity[k] > 1.001 { errors.push(format!("humidity out of range at {k}")); break; }
        if clim.soil_moisture[k] < -0.001 || clim.soil_moisture[k] > 1.001 { errors.push(format!("soil moisture out of range at {k}")); break; }
        if clim.snow[k] < -0.001 { errors.push(format!("negative snowpack at {k}")); break; }
        if clim.snow[k] > 0.0 && clim.monthly_temp[k.min(clim.monthly_temp.len().saturating_sub(1))] > 8.0 {
            errors.push(format!("snowpack inconsistent with warm temperature at {k}"));
            break;
        }
    }
    if height.len() != n || land.len() != n { errors.push("terrain field size mismatch".into()); return errors; }
    let mut row_mean = vec![0.0; h];
    let mut row_count = vec![0usize; h];
    for j in 0..h {
        for i in 0..w {
            let k = j*w+i;
            if land[k] { row_mean[j] += clim.temp[k] as f64; row_count[j] += 1; }
        }
        if row_count[j] > 0 { row_mean[j] /= row_count[j] as f64; }
    }
    if h >= 3 {
        let top = row_mean[0];
        let bottom = row_mean[h-1];
        let top_lat = row_mean[0];
        let _ = (top, bottom, top_lat);
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hemispheres_have_opposite_seasons() {
        let mut f = crate::WorldFile::default();
        f.params.width_mi = 600.0;
        f.params.height_mi = 600.0;
        f.params.lat_top = 60.0;
        f.params.lat_bottom = -60.0;
        let wld = crate::World::new(f).unwrap();
        let h = 17;
        let w = 17;
        let height = vec![0.0; w*h];
        let land = vec![true; w*h];
        let c = build(&wld, w, h, 20_000.0, &height, &land);
        let north_july = c.monthly_temp[6*w];
        let north_jan = c.monthly_temp[0];
        let south_july = c.monthly_temp[6*w + (h-1)*w];
        let south_jan = c.monthly_temp[(h-1)*w];
        assert!(north_july > north_jan);
        assert!(south_july < south_jan);
    }

    #[test]
    fn latitude_respects_configured_map_edges() {
        let mut f = crate::WorldFile::default();
        f.params.lat_top = 60.0;
        f.params.lat_bottom = -40.0;
        let w = crate::World::new(f).unwrap();
        assert!((latitude(&w, 0, 101) - 60.0).abs() < 1e-9);
        assert!((latitude(&w, 100, 101) + 40.0).abs() < 1e-9);
    }

    #[test]
    fn distance_reaches_coast_without_noise() {
        let d = distance_to(5, 1, &[false, false, true, false, false]);
        assert_eq!(d[2], 0.0);
        assert!((d[0] - 2.0).abs() < 1e-9);
        assert!((d[4] - 2.0).abs() < 1e-9);
    }
}

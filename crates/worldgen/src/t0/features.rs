            }
        }
    }
    for &c in &river.cells {
        blocked[c as usize] = true;
    }
    blocked[mouth] = false;

    // A river-dominated delta is a small distributary network: several channels share
    // the proximal trunk, then separate around mouth bars into a fan of unequal outlets.
    // Keep this deliberately small so generated maps get morphology, not a bundle of scratches.
    let branch_count = if strength >= 5.0 { 5 } else { 4 };
    let mut targets = Vec::<(f64, f64, usize)>::new();
    let target_angles = if branch_count == 5 {
        [-0.95_f64, -0.48, 0.0, 0.48, 0.95]
    } else {
        [-0.82_f64, -0.30, 0.30, 0.82]
    };
    let target_radius = radius as f64 * 0.82;

    for y in my as isize - radius as isize..=my as isize + radius as isize {
        if y < 0 || y >= h as isize {
            continue;
        }
        for x in mx as isize - radius as isize..=mx as isize + radius as isize {
            if x < 0 || x >= w as isize {
                continue;
            }
            let k = y as usize * w + x as usize;
            if inp.land[k] || !neighbors(w, h, k).any(|(nb, _)| inp.land[nb]) {
                continue;
            }

            let vx = x as f64 - mx;
            let vy = y as f64 - my;
            let d = crate::core::sqrt(vx * vx + vy * vy);
            if d < 5.0 || d > radius as f64 {
                continue;
            }

            let along = (vx * fx + vy * fy) / d;
            if along < 0.35 {
                continue;
            }

            let cross = fx * vy - fy * vx;
            let angle = libm::atan2(cross, vx * fx + vy * fy);
            if angle.abs() > 1.15 {
                continue;
            }

            let land_goal = neighbors(w, h, k)
                .filter(|(nb, _)| inp.land[*nb] && inp.hydro.lake_of[*nb] == super::hydro::NO_LAKE)
                .map(|(nb, _)| nb)
                .find(|&nb| !blocked[nb]);
            let Some(goal) = land_goal else { continue; };

            let coast_z = inp.height[goal];
            let grade = ((mouth_z - coast_z).max(0.0)) / (d * cell_ft).max(cell_ft);
            if !grade.is_finite() || grade > super::hydro::DELTA_MAX_GRADE {
                continue;
            }

            let target_angle_error = target_angles
                .iter()
                .map(|&a| (angle - a).abs())
                .fold(f64::INFINITY, f64::min);
            let distance_error = (d - target_radius).abs() / radius as f64;
            let target_score = target_angle_error * 9.0
                + distance_error * 4.0
                - along * 0.5;
            targets.push((target_score, angle, k));
        }
    }

    // Select one terminal mouth for each intended finger. The separation constraint prevents
    // adjacent shoreline pixels from becoming fake duplicate distributaries.
    let mut chosen = Vec::<usize>::with_capacity(branch_count);
    for &wanted_angle in &target_angles {
        let mut best: Option<(f64, usize)> = None;
        for &(score, angle, target) in &targets {
            if (angle - wanted_angle).abs() > 0.32 {
                continue;
            }
            if chosen.iter().any(|&other| {
                let dx = (target % w) as f64 - (other % w) as f64;
                let dy = (target / w) as f64 - (other / w) as f64;
                dx * dx + dy * dy < (0.34 * radius as f64).powi(2)
            }) {
                continue;
            }
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, target));
            }
        }
        if let Some((_, target)) = best {
            chosen.push(target);
        }
    }
    if chosen.len() < 3 {
        return None;
    }

    // Route all distributaries from the real mouth. Only the distal portion is reserved after
    // each route, so later branches naturally share the proximal trunk before bifurcating.
    let branch_start = mouth;
    let mut used = vec![false; w * h];
    let mut paths = Vec::with_capacity(chosen.len());

    for (branch_index, &target) in chosen.iter().enumerate() {
        let mut goals = Vec::<usize>::new();
        for (nb, _) in neighbors(w, h, target) {
            if inp.land[nb]
                && inp.hydro.lake_of[nb] == super::hydro::NO_LAKE
                && !blocked[nb]
            {
                goals.push(nb);
            }
        }
        goals.sort_unstable();

        let mut best: Option<Vec<usize>> = None;
        for goal in goals {
            if let Some(path) = delta_route(
                inp,
                branch_start,
                goal,
                &used,
                &blocked,
                radius + 7,
                fx,
                fy,
            ) {
                if path.len() < 5 {
                    continue;
                }
                let simplified = simplify_delta_path(inp, &path, &blocked);
                if simplified.len() < 4 {
                    continue;
                }
                if best.as_ref().is_none_or(|b| simplified.len() < b.len()) {
                    best = Some(simplified);
                }
            }
        }

        let Some(path) = best else { continue; };

        // Reserve only the downstream part. Natural deltas share a feeder trunk, then split
        // progressively farther from the apex around inter-distributary islands/bars.
        let reserve_from = path.len() * 38 / 100;
        for (i, &cell) in path.iter().enumerate().skip(reserve_from) {
            used[cell] = true;
            if i > reserve_from && i + 1 < path.len() {
                let prev = path[i - 1];
                let next = path[i + 1];
                let dx = (next % w) as isize - (prev % w) as isize;
                let dy = (next / w) as isize - (prev / w) as isize;
                let side = if branch_index % 2 == 0 { (-dy, dx) } else { (dy, -dx) };
                let sx = (cell % w) as isize + side.0.signum();
                let sy = (cell / w) as isize + side.1.signum();
                if sx >= 0 && sy >= 0 && sx < w as isize && sy < h as isize {
                    used[sy as usize * w + sx as usize] = true;
                }
            }
        }

        let mut points = path.iter()
            .map(|&k| [(k % w) as f64 * cell_ft, (k / w) as f64 * cell_ft])
            .collect::<Vec<_>>();
        points.push([(target % w) as f64 * cell_ft, (target / w) as f64 * cell_ft]);

        // Preserve a smooth, slightly sinuous distributary planform without applying the
        // ordinary river meander field a second time in mod.rs.
        let weights = vec![1.0f32; points.len()];
        let (smooth, _) = chaikin(&points, &weights, 2);
        if smooth.len() >= 4 {
            paths.push(smooth);
        }
    }

    if paths.len() >= 3 { Some(paths) } else { None }
}

fn simplify_delta_path(inp: &Inputs, path: &[usize], blocked: &[bool]) -> Vec<usize> {
    if path.len() <= 2 {
        return path.to_vec();
    }

    let mut out = Vec::with_capacity(path.len());
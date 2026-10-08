//! Drawing the night sky: the slow base layer, the stars, planets and
//! showpieces, the ring, the names beside things, the words, and the keys
//! in the corner.

use super::*;

impl Game {
    /// The slow-changing layer under the stars: the faint lattice, the air's
    /// glow near the horizon, twilight, the Milky Way and the ground. Worked
    /// out once per two-by-two block of dots, which it is smooth enough for.
    pub(super) fn rebuild_base(&mut self, hz: &Mat3, sun: Vec3, sun_alt: f64, limit: f64) {
        let cam = self.camera;
        let (cols, rows) = (self.field.cols, self.field.rows);
        let pitch = self.field.pitch as f64;
        let to_eq = transpose(hz);
        // The Milky Way's frame: the north galactic pole and the galactic centre (J2000).
        let ngp = unit(192.859_48, 27.128_25);
        let gc = unit(266.405, -28.936_17);
        let gy = [
            ngp[1] * gc[2] - ngp[2] * gc[1],
            ngp[2] * gc[0] - ngp[0] * gc[2],
            ngp[0] * gc[1] - ngp[1] * gc[0],
        ];
        let milky = ((limit - 3.2) / 2.3).clamp(0.0, 1.0);
        let twilight = ((sun_alt + 18.0) / 24.0).clamp(0.0, 1.0);
        let twilight = (twilight * twilight) as f32;
        let base = self.field.base_mut();
        for br in (0..rows).step_by(2) {
            for bc in (0..cols).step_by(2) {
                let (x, y) = ((bc as f64 + 1.0) * pitch, (br as f64 + 1.0) * pitch);
                let v = cam.unproject(x, y);
                let (alt, az) = alt_az(v);
                let mut milky_here = 0f32;
                let cell: Rgb = if alt < hills(az) {
                    let ground = 0.018 + 0.03 * twilight;
                    [ground * 0.9, ground * 0.85, ground * 0.8]
                } else {
                    let mut m = 0.0;
                    if milky > 0.0 {
                        let eq = apply(&to_eq, v);
                        let b = dot3(eq, ngp).clamp(-1.0, 1.0).asin().to_degrees();
                        let l = dot3(eq, gy).atan2(dot3(eq, gc));
                        let core = ((1.0 + l.cos()) / 2.0).powi(3);
                        let width = 6.5 + 7.0 * core;
                        let band = (-(b / width).powi(2)).exp();
                        if band > 0.01 {
                            let grain = noise3(eq[0] * 9.0, eq[1] * 9.0, eq[2] * 9.0) * 0.6
                                + noise3(eq[0] * 23.0, eq[1] * 23.0, eq[2] * 23.0) * 0.4;
                            let ld = l.to_degrees();
                            let rift = if (15.0..80.0).contains(&ld) {
                                0.55 * (-((b - 2.0) / 3.0).powi(2)).exp()
                            } else {
                                0.0
                            };
                            m = band
                                * (0.35 + 0.65 * core)
                                * (0.45 + 0.9 * grain)
                                * (1.0 - rift)
                                * 0.14
                                * milky;
                        }
                    }
                    let m = m as f32;
                    milky_here = m;
                    let lattice = 0.024f32;
                    let air = (0.03 * (-(alt / 9.0)).exp()) as f32;
                    let sunward = ((dot3(v, sun) + 1.0) / 2.0).powi(3) as f32;
                    let dusk = twilight * (0.18 + 0.5 * sunward);
                    [
                        lattice * 0.8 + air * 0.7 + dusk * (0.45 + 0.5 * sunward) + m * 0.82,
                        lattice * 0.88 + air * 0.8 + dusk * (0.55 + 0.2 * sunward) + m * 0.87,
                        lattice * 1.1 + air * 1.0 + dusk * (0.95 - 0.3 * sunward) + m,
                    ]
                };
                for r in br..(br + 2).min(rows) {
                    for c in bc..(bc + 2).min(cols) {
                        base[r * cols + c] = cell;
                    }
                }
                // The Milky Way as a dust of faint dots rather than a fog: each
                // dot takes a share by where it points on the sky, so the dust
                // stays put as the view moves.
                // Zoomed in, the dust's cells would show as blotches: let it go smooth.
                let dusty = smoothstep((cam.fov - 8.0) / 30.0) as f32;
                if milky_here > 0.002 && dusty > 0.0 {
                    for r in br..(br + 2).min(rows) {
                        for c in bc..(bc + 2).min(cols) {
                            let v =
                                cam.unproject((c as f64 + 0.5) * pitch, (r as f64 + 0.5) * pitch);
                            let eq = apply(&to_eq, v);
                            let cell_of = |x: f64, prime: i64| (x * 420.0).floor() as i64 * prime;
                            let k = cell_of(eq[0], 73_856_093)
                                ^ cell_of(eq[1], 19_349_663)
                                ^ cell_of(eq[2], 83_492_791);
                            let h = ((k as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40) as f32
                                / (1u64 << 24) as f32;
                            let share = (0.15 + 2.2 * h * h * h) / 0.7 - 1.0;
                            let add = milky_here * share * dusty;
                            let cell = &mut base[r * cols + c];
                            cell[0] = (cell[0] + add * 0.82).max(0.0);
                            cell[1] = (cell[1] + add * 0.87).max(0.0);
                            cell[2] = (cell[2] + add).max(0.0);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn draw(
        &mut self,
        real: UnixMs,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
        tempo: f64,
        dt: f64,
    ) -> Frame {
        let cam = self.camera;
        // Fine enough that the lattice reads as texture, not as pixels.
        let pitch = if cam.width > 2200.0 { 3.2 } else { 2.8 };
        let resized = self.field.fit(cam.width, cam.height, pitch);
        let sun_seen = see(Body::Sun, self.observer, now);
        let sun_alt = sun_seen.alt;
        let sun = from_alt_az(sun_seen.alt, sun_seen.az);
        let limit = limiting_magnitude(sun_alt);
        let lapsing =
            self.session.lapse_progress(real) > 0.0 && self.session.lapse_progress(real) < 1.0;
        let stamp = BaseStamp {
            az: (cam.az * 50.0).round(),
            alt: (cam.alt * 50.0).round(),
            fov: (cam.fov * 200.0).round(),
            width: cam.width,
            height: cam.height,
            sky_minute: now / 20_000,
            lapse: lapsing,
        };
        if resized || self.base != Some(stamp) || lapsing {
            self.rebuild_base(hz, sun, sun_alt, limit);
            self.base = Some(stamp);
        }
        self.field.begin();
        self.points.clear();
        let t = real as f64 / 1000.0;
        let arrival = if self.session.phase() == Phase::Arrival {
            1.0 - smoothstep(
                self.session.in_phase(real) as f64 / self.session.timings.arrival as f64,
            )
        } else {
            0.0
        };

        // Stars.
        let ppd = cam.px_per_degree();
        let shown = limit.min(5.6) + 0.6;
        let low_sin = 14f64.to_radians().sin();
        let algol = self
            .algol
            .map(|k| (k, westering_core::tours::algol_magnitude(now)));
        // Stars well outside the view are passed over with one dot product
        // in their own frame, before the turn into the horizon's.
        let half_diag =
            (cam.width.hypot(cam.height) / 2.0 + 80.0) / (ppd * 180.0 / std::f64::consts::PI);
        let reach = (2.0 * (half_diag / 2.0).atan()).min(std::f64::consts::PI);
        let cos_reach = reach.cos() - 0.02;
        let ahead = apply(&transpose(hz), from_alt_az(cam.alt, cam.az));
        for (k, star) in self.prepared.iter().enumerate() {
            if star.mag > shown {
                break;
            }
            if dot3(star.dir, ahead) < cos_reach {
                continue;
            }
            let (mag, star_light) = match algol {
                Some((a, m)) if a == k => (m, light(m)),
                _ => (star.mag, star.light),
            };
            let mut v = apply(hz, star.dir);
            if v[2] < -0.02 {
                continue;
            }
            if v[2] < 0.06 {
                let (alt, az) = alt_az(v);
                let lifted = alt + refraction(alt);
                if lifted < hills(az) {
                    continue;
                }
                v = from_alt_az(lifted, az);
            }
            let Some((mut x, mut y)) = cam.project(v) else {
                continue;
            };
            if !cam.on_screen(x, y, 60.0) {
                continue;
            }
            if arrival > 0.0 {
                let spread = arrival * arrival * 70.0;
                x += (star.phase * 12.9898).sin() * spread;
                y += (star.phase * 78.233).sin() * spread;
            }
            let fade = smoothstep((shown - star.mag) / 1.2) as f32;
            let low = 0.3 + 0.7 * smoothstep(v[2] / low_sin);
            let rate = star.rate * tempo;
            let depth = (0.12 + 0.22 * (1.0 - v[2]).powi(2))
                * if self.journal.settings.calm {
                    0.35
                } else {
                    1.0
                };
            let twinkle = 1.0
                + depth
                    * ((t * rate + star.phase).sin() * 0.6
                        + (t * rate * 1.7 + star.phase * 0.37).sin() * 0.4);
            let amount = star_light * fade * (low * twinkle) as f32;
            if mag < BRIGHT {
                self.points.push(point(x, y, star.tint, amount, 1.0));
            } else {
                // Single dots on the fine lattice need a little more light to hold the eye.
                self.field.star(x, y, star.tint, amount * 1.45);
            }
        }

        // A photograph taking over from the dots it replaces.
        self.update_eye(now, hz, prec, dt);
        let (photo_body, photo_piece) = match self.eye.map(|e| self.eye_of(e)) {
            Some(Target::Body(b)) => (Some(b), None),
            Some(Target::Showpiece(p)) => (None, Some(p)),
            _ => (None, None),
        };
        let keep = (1.0 - self.eye_alpha) as f32;

        // Showpieces' soft light. Zoomed in, the view gathers light like a
        // telescope and fainter things show.
        let gather = gather(cam.fov);
        for (pi, piece) in self.sky.lists.showpieces.iter().enumerate() {
            let fade = if photo_piece == Some(pi) { keep } else { 1.0 };
            if piece.mag > limit + gather + 0.5
                || !matches!(
                    piece.kind,
                    Kind::Cluster | Kind::Cloud | Kind::Galaxy | Kind::Nebula
                )
            {
                continue;
            }
            let v = apply(hz, apply(prec, unit(piece.ra, piece.dec)));
            if alt_az(v).0 < 1.0 {
                continue;
            }
            let Some((x, y)) = cam.project(v) else {
                continue;
            };
            if !cam.on_screen(x, y, 200.0) {
                continue;
            }
            let true_radius = piece.size / 60.0 / 2.0 * ppd;
            let radius = true_radius.max(pitch as f64 * 0.8);
            let strength = (10f64.powf(-0.4 * (piece.mag - gather - 3.0))).min(2.0) as f32;
            // How comfortably it's within reach: faint things just in reach
            // still show, rather than fading into the lattice.
            let seen = smoothstep((limit + gather + 0.5 - piece.mag) / 1.5) as f32;
            // A star cloud is drawn as a cluster is: many faint stars.
            let crowd = matches!(piece.kind, Kind::Cluster | Kind::Cloud);
            if !crowd && (true_radius < 9.0 || piece.size < 3.0) {
                // Small nebulae and galaxies: a soft point, a little disc close up.
                let color = if piece.kind == Kind::Nebula {
                    [0.62, 0.95, 0.92]
                } else {
                    [0.95, 0.93, 0.86]
                };
                self.points.push(crate::view::Point {
                    x,
                    y,
                    radius: (true_radius as f32).clamp(1.3, 40.0),
                    color,
                    alpha: (0.3 + 0.4 * seen) * fade * if true_radius > 9.0 { 0.8 } else { 1.0 },
                    halo: 0.9 * seen * fade,
                });
                continue;
            }
            if crowd {
                // Stars too faint to see one by one, sprinkled as single dots.
                let count = (piece.size / 4.0).clamp(8.0, 36.0) as usize;
                let seed = piece
                    .id
                    .bytes()
                    .fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
                for k in 0..count {
                    let h1 = ((seed.wrapping_add(k as u64 * 7919)) as f64 * 0.618_034).fract();
                    let h2 = ((seed.wrapping_add(k as u64 * 104_729)) as f64 * 0.414_214).fract();
                    let rr = radius * h1.sqrt();
                    let a = h2 * std::f64::consts::TAU;
                    let twinkle = 0.7 + 0.3 * (t * 1.3 * tempo + k as f64).sin();
                    self.field.dot(
                        x + rr * a.cos(),
                        y + rr * a.sin(),
                        [0.85, 0.9, 1.0],
                        0.07 * strength * twinkle as f32 * fade,
                    );
                }
            } else {
                let amount = (0.05 * strength).min(0.12).max(0.035 * seen);
                let amount = if radius > 120.0 {
                    amount * 120.0 / radius as f32
                } else {
                    amount
                };
                let color = if piece.kind == Kind::Nebula {
                    [0.8, 0.95, 0.95]
                } else {
                    [0.95, 0.93, 0.88]
                };
                self.field.glow(x, y, radius, color, amount * fade);
            }
        }

        // The Sun, the Moon and the planets.
        for body in [
            Body::Sun,
            Body::Moon,
            Body::Mercury,
            Body::Venus,
            Body::Mars,
            Body::Jupiter,
            Body::Saturn,
        ] {
            let s = if body == Body::Sun {
                sun_seen
            } else {
                see(body, self.observer, now)
            };
            if s.alt < hills(s.az) - 0.3 {
                continue;
            }
            let v = from_alt_az(s.alt, s.az);
            let Some((x, y)) = cam.project(v) else {
                continue;
            };
            if !cam.on_screen(x, y, 120.0) {
                continue;
            }
            let radius = s.position.diameter / 3600.0 / 2.0 * ppd;
            let fade = if photo_body == Some(body) { keep } else { 1.0 };
            match body {
                Body::Sun => {
                    self.field
                        .glow(x, y, radius.max(pitch as f64 * 2.2), [1.0, 0.95, 0.82], 3.0)
                }
                Body::Moon => {
                    // Drawn larger than life when the view is wide, so it reads as round.
                    let r = radius.max(pitch as f64 * 5.5);
                    self.lit_disc(
                        x,
                        y,
                        r,
                        v,
                        sun,
                        180.0 - s.position.elongation,
                        [1.0, 0.97, 0.9],
                        1.05 * fade,
                        true,
                    );
                }
                _ => {
                    let color = match body {
                        Body::Mars => [1.0, 0.66, 0.5],
                        Body::Jupiter => [1.0, 0.95, 0.86],
                        Body::Saturn => [1.0, 0.9, 0.72],
                        _ => [1.0, 0.97, 0.92],
                    };
                    if radius > pitch as f64 * 1.2 {
                        let phase_angle = s
                            .position
                            .phase
                            .mul_add(2.0, -1.0)
                            .clamp(-1.0, 1.0)
                            .acos()
                            .to_degrees();
                        self.lit_disc(x, y, radius, v, sun, phase_angle, color, 1.5 * fade, false);
                    } else {
                        let low = 0.4 + 0.6 * smoothstep(s.alt / 12.0);
                        let amount = light(s.position.magnitude) * 1.08 * low as f32;
                        let mut p = point(x, y, color, amount, 1.3);
                        p.alpha *= fade;
                        p.halo *= fade;
                        self.points.push(p);
                    }
                }
            }
        }

        // Jupiter's four big moons, close up.
        self.moon_labels.clear();
        let jup = see(Body::Jupiter, self.observer, now);
        if cam.fov < 1.5 && jup.alt > 0.0 {
            let v = from_alt_az(jup.alt, jup.az);
            if let Some((jx, jy)) = cam.project(v) {
                let toward = |w: Vec3| {
                    let along = dot3(w, v);
                    let t = [
                        w[0] - along * v[0],
                        w[1] - along * v[1],
                        w[2] - along * v[2],
                    ];
                    let n = dot3(t, t).sqrt().max(1e-12);
                    let nudge = [
                        v[0] + t[0] / n * 1e-5,
                        v[1] + t[1] / n * 1e-5,
                        v[2] + t[2] / n * 1e-5,
                    ];
                    cam.project(nudge).map(|(a, b)| {
                        let (dx, dy) = (a - jx, b - jy);
                        let l = (dx * dx + dy * dy).sqrt().max(1e-12);
                        (dx / l, dy / l)
                    })
                };
                let ra = jup.ra.to_radians();
                let east = apply(hz, [-ra.sin(), ra.cos(), 0.0]);
                let pole = apply(hz, apply(prec, westering_core::jupiter::pole()));
                if let (Some(e), Some(p)) = (toward(east), toward(pole)) {
                    // Along Jupiter's equator, the way that points to the sky's west.
                    let mut q = (-p.1, p.0);
                    if q.0 * -e.0 + q.1 * -e.1 < 0.0 {
                        q = (-q.0, -q.1);
                    }
                    let r = jup.position.diameter / 7200.0 * ppd;
                    let a = smoothstep((1.5 - cam.fov) / 1.0) as f32;
                    for (k, m) in westering_core::jupiter::moons(now).iter().enumerate() {
                        if m.behind && m.x.abs() < 1.0 {
                            continue;
                        }
                        let (mx, my) = (
                            jx + (m.x * q.0 + m.y * p.0) * r,
                            jy + (m.x * q.1 + m.y * p.1) * r,
                        );
                        let mut dot = point(mx, my, [1.0, 0.97, 0.9], 0.9, 0.6);
                        dot.alpha *= a;
                        dot.halo *= a;
                        dot.radius = dot.radius.min(2.2);
                        self.points.push(dot);
                        self.moon_labels
                            .push((mx, my, westering_core::jupiter::NAMES[k], a));
                    }
                }
            }
        }

        // Meteors.
        for m in &self.meteors {
            let age = (real - m.start) as f64 / m.duration as f64;
            if !(0.0..=1.3).contains(&age) {
                continue;
            }
            let head = age.min(1.0) * m.length;
            let tail = (head - m.length * 0.4).max(0.0);
            let fade = if age > 1.0 {
                1.0 - (age - 1.0) / 0.3
            } else {
                1.0
            } as f32;
            let at = |th: f64| {
                [
                    m.from[0] * th.cos() + m.toward[0] * th.sin(),
                    m.from[1] * th.cos() + m.toward[1] * th.sin(),
                    m.from[2] * th.cos() + m.toward[2] * th.sin(),
                ]
            };
            let steps = 18;
            let mut prev: Option<(f64, f64)> = None;
            for k in 0..=steps {
                let f = k as f64 / steps as f64;
                let th = tail + (head - tail) * f;
                let p = cam.project(at(th));
                if let (Some(a), Some(b)) = (prev, p) {
                    self.field.line(
                        a,
                        b,
                        [0.9, 0.95, 1.0],
                        m.brightness * fade * (0.08 + 0.5 * f as f32),
                    );
                }
                prev = p;
            }
        }

        self.draw_marks(real, hz);
        self.marks.clear();
        self.draw_tours(real, hz, prec);
        let lines = self.pattern_lines(real, hz, dt);
        self.draw_reticle(real, tempo);

        let brightness = self.session.brightness(real);
        let frame_texture = self.field.texture(brightness as f32);
        let mut texts = if self.keeping() {
            Vec::new()
        } else {
            self.labels(real, now, hz, prec, brightness)
        };
        texts.extend(self.mark_labels(hz, brightness));
        texts.extend(self.plan_mark_frame(now, real));
        for &(x, y, name, a) in &self.moon_labels {
            texts.push(Text::new(
                x + 8.0,
                y + 6.0,
                name,
                12.0,
                0.55 * a as f64 * brightness,
            ));
        }
        texts.extend(self.words(real, brightness));
        if !self.keeping() {
            self.hover_scan(now, hz, prec);
        }
        self.sprite_brightness = brightness;
        let (sprites, bubble, embers) = self.guide_frame(real, brightness);
        // Looking at something close up, its card tells all this instead.
        if let Some(eye) = self.eye
            && self.inspect.is_none()
            && real > self.backing_out
            && self.eye_alpha > 0.05
            && let Some(c) = self
                .eye_photo(eye)
                .and_then(|id| self.photos.credit(&id).cloned())
        {
            // Laid out from the bottom up: the credit, and above it what the
            // picture shows that an eye wouldn't.
            let credit = format!("Photograph: {}", c.credit);
            let lines = |text: &str, per_line: f64| (text.chars().count() as f64 / per_line).ceil();
            let credit_y = cam.height - 30.0 - lines(&credit, 62.0) * 15.0;
            texts.push(
                Text::new(
                    cam.width - 420.0,
                    credit_y,
                    credit,
                    11.5,
                    0.45 * self.eye_alpha,
                )
                .wrap(400.0),
            );
            if let Some(caption) = c.caption {
                let y = credit_y - 10.0 - lines(&caption, 50.0) * 20.0;
                texts.push(
                    Text::new(cam.width - 420.0, y, caption, 14.5, 0.8 * self.eye_alpha)
                        .wrap(400.0)
                        .color([0.95, 0.92, 0.85]),
                );
            }
        }
        self.marks.extend(embers);
        let eyepiece = self.eyepiece(now, hz, prec);
        let card = self.card_view(real);
        let compass = (matches!(
            self.session.phase(),
            Phase::Arrival | Phase::Weights | Phase::Hunt | Phase::Dimming
        ) && self.keep.mix < 0.5)
            .then(|| crate::view::Compass {
                heading: cam.az,
                alpha: brightness.max(0.5),
                bottom: compact(cam.width, cam.height),
            });
        Frame {
            glows: Vec::new(),
            legend: self.legend(dt),
            silhouettes: Vec::new(),
            eyepiece,
            card,
            compass,
            points: std::mem::take(&mut self.points),
            marks: std::mem::take(&mut self.marks),
            lines,
            sprites,
            bubble,
            texture: Some(frame_texture),
            cols: self.field.cols,
            rows: self.field.rows,
            pitch: self.field.pitch,
            background: [0.012, 0.014, 0.024],
            texts,
            veil: 0.0,
        }
    }

    /// The keys in the corner while looking round the sky, stepping aside
    /// for anything else that needs the room.
    pub(super) fn legend(&mut self, dt: f64) -> Option<crate::view::Legend> {
        if let Some(table) = self.help_table(self.last_real) {
            self.legend_mix = 0.0;
            return Some(table);
        }
        let wanted = self.hunting()
            && !self.keeping()
            && !self.winding()
            && self.card.is_none()
            && self.talk.prompt.is_none()
            && self.drawing.is_none()
            && self.tour.is_none()
            && self.eye_alpha < 0.05
            && !compact(self.camera.width, self.camera.height);
        let target = if wanted { 1.0 } else { 0.0 };
        self.legend_mix += (target - self.legend_mix) * (1.0 - (-dt / 0.4).exp());
        (self.legend_mix > 0.01).then(|| crate::view::Legend {
            rows: if self.free_look() {
                vec![
                    (vec!["Drag".into()], "look around".into()),
                    (vec!["Scroll".into()], "zoom in and out".into()),
                    (
                        vec!["Click".into()],
                        "a closer look at anything that glows".into(),
                    ),
                    (vec!["F".into()], "the guided way, with the wisp".into()),
                    (vec!["?".into()], "all the keys".into()),
                ]
            } else {
                let mut rows = vec![
                    (vec!["+".into(), "−".into()], "zoom in and out".into()),
                    (vec!["← ↑ ↓ →".into()], "aim the ring".into()),
                    (vec!["Space".into()], "carry on; hold to catch".into()),
                    (vec!["Tab".into()], "back to what you skipped".into()),
                ];
                // Free look only once the wisp has handed the sky over.
                if self.handed_over {
                    rows.push((vec!["F".into()], "free look, with the mouse".into()));
                }
                rows.push((vec!["?".into()], "all the keys".into()));
                rows
            },
            alpha: self.legend_mix,
            whole: false,
        })
    }

    /// Every key and what it does, as a table in the middle of the screen,
    /// while it's asked for and as it fades away after.
    pub(crate) fn help_table(&self, real: UnixMs) -> Option<crate::view::Legend> {
        let since = smoothstep((real - self.help_changed) as f64 / 220.0);
        let alpha = if self.help { since } else { 1.0 - since };
        if alpha < 0.01 {
            return None;
        }
        let row = |keys: &[&str], what: &str| {
            (
                keys.iter().map(|k| (*k).to_owned()).collect::<Vec<_>>(),
                what.to_owned(),
            )
        };
        let mut rows = if self.by_day() {
            vec![
                row(&["Click"], "Anything on the ground, or on the list"),
                row(&["Space", "Enter"], "Carry on, a card at a time"),
                row(&["Esc"], "Put a card away"),
            ]
        } else if self.free_look() {
            vec![
                row(&["Drag"], "Look around"),
                row(&["Scroll"], "Zoom in and out"),
                row(&["Click"], "A closer look at anything that glows"),
                row(&["Esc"], "Back out again"),
                row(&["F"], "The guided way, with the wisp"),
            ]
        } else {
            vec![
                row(&["← ↑ ↓ →"], "Look around (or drag)"),
                row(&["+", "−"], "Zoom in and out (or scroll)"),
                row(&["Space", "Enter"], "Carry on: the next word, card or find"),
                row(&["Hold Space"], "Catch what's in the ring"),
                row(&["Tab"], "Back to anything you skipped"),
                row(&["Click"], "Something on the list, to turn to it"),
                row(&["Esc"], "Put a card away"),
                row(
                    &["F"],
                    if self.handed_over {
                        "Free look, with the mouse"
                    } else {
                        "Free look, once we've looked together a while"
                    },
                ),
                row(&["C"], "Draw a constellation of your own"),
            ]
        };
        rows.extend([
            row(&["L"], "The logbook"),
            row(&["M"], "Music: the next style, then off"),
            row(&["K"], "Keep me company in the background"),
            row(
                &["W"],
                if self.by_day() {
                    "Back to your day"
                } else {
                    "Wind down for the night"
                },
            ),
            row(&["Ctrl", ","], "Settings"),
            row(&["F11"], "Full screen"),
            row(&["?"], "These keys, and away again"),
        ]);
        Some(crate::view::Legend {
            rows,
            alpha,
            whole: true,
        })
    }

    pub(super) fn draw_reticle(&mut self, real: UnixMs, tempo: f64) {
        let ringed = matches!(self.session.phase(), Phase::Hunt | Phase::Arrival) || self.placing();
        if !ringed
            || self.card.is_some()
            || self.keeping()
            || (self.free_look() && !self.placing())
            || self.ring_resting
        {
            return;
        }
        let r = self.reticle_radius();
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        let has = self.catch.target.is_some() || self.placing();
        let breathe = 0.5 + 0.5 * ((real as f64 / 1000.0) * 1.1 * tempo).sin();
        let points = 28;
        for k in 0..points {
            let f = k as f64 / points as f64;
            // Four arcs with gaps at the compass points.
            let quarter = (f * 4.0).fract();
            if !(0.12..0.88).contains(&quarter) {
                continue;
            }
            let a = f * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
            let (x, y) = (cx + r * a.cos(), cy + r * a.sin());
            let filled = f < self.catch.progress;
            let (color, amount) = if filled {
                (WARM, 0.7)
            } else if has {
                (RETICLE, 0.22 + 0.1 * breathe as f32)
            } else {
                (RETICLE, 0.09)
            };
            self.field.splat(x, y, color, amount);
        }
    }

    /// Names beside things near the reticle, and the compass on the horizon.
    pub(super) fn labels(
        &self,
        real: UnixMs,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
        brightness: f64,
    ) -> Vec<Text> {
        let mut out = Vec::new();
        let cam = &self.camera;
        let (cx, cy) = (cam.width / 2.0, cam.height / 2.0);
        let r = self.reticle_radius();
        if matches!(
            self.session.phase(),
            Phase::Hunt | Phase::Arrival | Phase::Dimming
        ) {
            for (name, az) in [
                ("N", 0.0),
                ("E", 90.0),
                ("S", 180.0),
                ("W", 270.0),
                ("NE", 45.0),
                ("SE", 135.0),
                ("SW", 225.0),
                ("NW", 315.0),
            ] {
                if let Some((x, y)) = cam.project(from_alt_az(hills(az) + 0.8, az))
                    && cam.on_screen(x, y, -10.0)
                {
                    let (size, alpha) = if name.len() == 1 {
                        (17.0, 0.6)
                    } else {
                        (12.0, 0.4)
                    };
                    let mut label =
                        Text::new(x, y - 26.0, name, size, alpha * brightness).centred();
                    if name.len() == 1 {
                        label = label.bold();
                    }
                    if name == "N" {
                        label = label.color([1.0, 0.8, 0.58]);
                    }
                    out.push(label);
                }
            }
        }
        if self.hunting() && self.card.is_none() {
            if let Some(i) = self.catch.target
                && let Some(v) = self.find_dir(i, now, hz, prec)
                && let Some((x, y)) = cam.project(v)
            {
                let near = 1.0 - (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() / r).min(1.0);
                let words = if self.catch.progress > 0.0 {
                    self.finds[i].name.clone()
                } else {
                    format!("{} · hold Space, or hold a click", self.finds[i].name)
                };
                let lx = cx + r.max(self.apparent_radius(i, cam.fov).min(r * 1.6)) + 14.0;
                out.push(Text::new(
                    lx,
                    cy - 9.0,
                    words,
                    14.0,
                    (0.35 + 0.5 * near) * brightness,
                ));
            }
            // Caught things keep a quiet name when the view passes them,
            // unless the ring is already labelling something there.
            for (i, f) in self.finds.iter().enumerate() {
                if !self.caught[i] || self.catch.target.is_some() {
                    continue;
                }
                if let Some(v) = self.find_dir(i, now, hz, prec)
                    && alt_az(v).0 > 0.0
                    && let Some((x, y)) = cam.project(v)
                {
                    let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                    if d < r * 2.5 {
                        let a = 0.3 * (1.0 - d / (r * 2.5));
                        out.push(Text::new(
                            x + 12.0,
                            y + 6.0,
                            f.name.clone(),
                            12.0,
                            a * brightness,
                        ));
                    }
                }
            }
        }
        let _ = real;
        out
    }

    /// The caption, the card, the hint, the last line and tonight's dots.
    pub(crate) fn words(&self, real: UnixMs, brightness: f64) -> Vec<Text> {
        let mut out = Vec::new();
        let (w, h) = (self.camera.width, self.camera.height);
        let text_light = brightness.max(0.55);
        if self.talk.caring {
            out.push(
                Text::new(w - 400.0, h - 130.0, westering_core::care::NOTE, 13.0, 0.75)
                    .wrap(360.0)
                    .color([1.0, 0.9, 0.8]),
            );
        }
        // The weight being hung keeps its words beside it.
        if let Some(words) = self.placing_words() {
            out.push(
                Text::new(w / 2.0 + 18.0, h / 2.0 - 10.0, words, 16.0, 0.85)
                    .wrap(320.0)
                    .color([1.0, 0.84, 0.68]),
            );
        }
        let hint = self
            .hint
            .as_ref()
            .map(|h| (h, envelope(real - h.shown, 1_000, h.hold, 1_800)));
        // Keeping company, the foot of the window is the wisp's: a word
        // about a setting takes the quiet line's place for a moment, one
        // going before the other comes.
        let told = match hint {
            Some((_, a)) if self.keeping() => a,
            _ => 0.0,
        };
        let quiet = 0.45 * self.keep.mix * (1.0 - told * 2.0).max(0.0);
        if quiet > 0.0 {
            // One quiet line, clear of the menu button in a narrow window.
            out.push(
                Text::new(
                    w / 2.0,
                    20.0,
                    match &self.playing {
                        Some((style, track)) => format!(
                            "Keeping you company · ♪ {style}: {track} · K brings the sky back"
                        ),
                        None => "Keeping you company · K brings the sky back".to_owned(),
                    },
                    12.0,
                    quiet,
                )
                .centred()
                .wrap((w - 140.0).max(120.0)),
            );
        }
        if let Some(c) = &self.caption {
            let a = envelope(real - c.shown, 1_500, c.hold, 2_000);
            // Below the panels at the top, in a small window.
            let y = if compact(w, h) { 120.0 } else { 62.0 };
            // A very short window has no room for it.
            if h >= 420.0 {
                out.push(
                    Text::new(w / 2.0, y, c.text.clone(), 16.0, 0.85 * a * text_light)
                        .centred()
                        .wrap(w * 0.8),
                );
            }
        }
        if let Some((hint, a)) = hint {
            if self.keeping() {
                let a = (told * 2.0 - 1.0).max(0.0);
                out.push(
                    Text::new(w / 2.0, 18.0, hint.text.clone(), 15.0, 0.9 * a)
                        .centred()
                        .wrap((w - 140.0).max(120.0)),
                );
            } else {
                // Above the compass strip, when that's along the bottom.
                let y = if compact(w, h) { h - 62.0 } else { h - 44.0 };
                out.push(Text::new(w / 2.0, y, hint.text.clone(), 13.0, 0.5 * a).centred());
            }
        }
        if let Some(handoff) = &self.handoff
            && self.session.last_line(real)
        {
            let into = self.session.in_phase(real) - self.session.timings.lapse - 900;
            let a = if self.session.phase() == Phase::LightsOut {
                1.0 - smoothstep(
                    self.session.in_phase(real) as f64 / self.session.timings.lights_out as f64
                        * 1.4,
                )
            } else {
                smoothstep(into as f64 / 4_000.0)
            };
            // Quiet, in the dimmed sky's own light: nothing brightens now.
            out.push(
                Text::new(w / 2.0, h * 0.7, handoff.line.clone(), 20.0, 0.55 * a)
                    .centred()
                    .wrap((w * 0.7).min(760.0))
                    .color([0.9, 0.84, 0.74]),
            );
        }
        out
    }
}

//! Photographs: laid over the sky at their true size as the view closes
//! in, or shown in a card; the Moon and inner planets lit to tonight's
//! phase.

use super::*;

impl Game {
    /// The id of a photograph of find `i` to lay over the sky, if there
    /// is one.
    pub(crate) fn photo_id(&self, i: usize) -> Option<String> {
        let id = match self.finds[i].target {
            Target::Body(b) => b.id().to_owned(),
            Target::Showpiece(p) => self.sky.lists.showpieces[p].id.clone(),
            _ => return None,
        };
        self.photos.credit(&id).filter(|c| !c.card_only).map(|_| id)
    }

    /// A photograph for find `i`'s card: a constellation's wide field, a
    /// star's close-up, or anything else pictured that can't be laid over
    /// the sky at its true size.
    pub(super) fn card_picture(&self, i: usize) -> Option<String> {
        let credit = match self.finds[i].target {
            Target::Figure(f) => self.photos.of_constellation(&self.sky.figures[f].abbrev),
            Target::Star(hr) => return self.star_picture(hr).map(|(id, _)| id),
            Target::Showpiece(p) => self.photos.credit(&self.sky.lists.showpieces[p].id),
            _ => None,
        }?;
        credit.card_only.then(|| credit.id.clone())
    }

    /// The photograph for a named star's card: its own close-up if it has
    /// one, and otherwise its constellation's wide field, with a line to
    /// say that's what the picture is.
    pub(super) fn star_picture(&self, hr: u16) -> Option<(String, Option<String>)> {
        let named = self.sky.lists.star_name(hr)?;
        if let Some(own) = self.photos.credit(&named.name.to_lowercase()) {
            return own.card_only.then(|| (own.id.clone(), None));
        }
        let wide = self.photos.of_constellation(&named.constellation)?;
        let figure = self
            .sky
            .figures
            .iter()
            .find(|f| f.abbrev == named.constellation)?;
        Some((
            wide.id.clone(),
            Some(format!(
                "The photograph is of {}, where {} is; the lines and the name are drawn on.",
                figure.name, named.name
            )),
        ))
    }

    /// How wide a showpiece's photograph is on the sky, in degrees.
    pub(super) fn photo_degrees(&self, p: usize) -> f64 {
        let piece = &self.sky.lists.showpieces[p];
        if let Some(d) = self.photos.credit(&piece.id).and_then(|c| c.degrees) {
            return d;
        }
        match piece.kind {
            // Pairs and single stars are pictured at a small telescope's scale.
            Kind::Double | Kind::Star => 0.3,
            _ => (piece.size / 60.0).max(0.1) / 0.8,
        }
    }

    /// Half the width of find `i`'s photograph on the screen, at a field of view.
    pub(crate) fn photo_half(&self, i: usize, fov: f64) -> f64 {
        self.target_half(&self.finds[i].target, fov)
    }

    /// Half the width of a photograph of `target` on the screen.
    pub(super) fn target_half(&self, target: &Target, fov: f64) -> f64 {
        let ppd = (self.camera.width / 2.0) / (2.0 * (fov.to_radians() / 4.0).tan())
            * std::f64::consts::PI
            / 180.0;
        let degrees = match *target {
            Target::Body(body) => {
                // Saturn's picture is framed by its rings, 2.27 times the
                // planet's width: the planet fills 38 per cent of it.
                let share = if body == Body::Saturn {
                    0.38
                } else {
                    crate::eyepiece::DISC
                };
                see(body, self.observer, self.clock.sky(self.last_real))
                    .position
                    .diameter
                    / 3600.0
                    / share
            }
            Target::Showpiece(p) => self.photo_degrees(p),
            _ => 0.0,
        };
        degrees / 2.0 * ppd
    }

    /// Which found thing's photograph to show: the one nearest the middle of
    /// the view, among those close enough to be worth a picture.
    pub(super) fn eye_target(&self, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<(Eye, f64)> {
        if !matches!(self.session.phase(), Phase::Hunt | Phase::Dimming) || self.drawing.is_some() {
            return None;
        }
        // Whatever's being looked at close up comes first.
        if let Some(eye) = self
            .inspect
            .as_ref()
            .and_then(|i| self.inspect_eye(i.subject))
            && let Some(v) = self.eye_dir(eye, now, hz, prec)
            && alt_az(v).0 > 0.0
        {
            let half = self.target_half(&self.eye_of(eye), self.camera.fov);
            return Some((eye, smoothstep((half - 12.0) / 50.0))).filter(|e| e.1 > 0.0);
        }
        let (cx, cy) = (self.camera.width / 2.0, self.camera.height / 2.0);
        // Tonight's finds once caught, and anything else pictured that the
        // view has closed in on: a telescope would show it too.
        let finds = (0..self.finds.len())
            .filter(|&i| self.caught[i] && self.photo_id(i).is_some())
            .map(Eye::Find);
        let listed = |p: usize| self.finds.iter().any(|f| f.target == Target::Showpiece(p));
        let pieces = (0..self.sky.lists.showpieces.len())
            .filter(|&p| !listed(p))
            .filter(|&p| {
                self.photos
                    .credit(&self.sky.lists.showpieces[p].id)
                    .is_some_and(|c| !c.card_only)
            })
            .map(Eye::Piece);
        finds
            .chain(pieces)
            .filter_map(|eye| {
                let v = self.eye_dir(eye, now, hz, prec)?;
                if alt_az(v).0 < 0.0 {
                    return None;
                }
                let (x, y) = self.camera.project(v)?;
                let target = self.eye_of(eye);
                let half = self.target_half(&target, self.camera.fov);
                let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                // Worth showing once the picture is big enough to hold detail:
                // a planet's disc sooner than a spread-out cluster.
                let size = match target {
                    Target::Body(_) => smoothstep((half - 15.0) / 60.0),
                    _ => smoothstep((half - 40.0) / 160.0),
                };
                (size > 0.0 && d < half + self.camera.width * 0.5).then_some((eye, size, d))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(eye, size, _)| (eye, size))
    }

    /// What a photograph is of, as a find's target.
    pub(super) fn eye_of(&self, eye: Eye) -> Target {
        match eye {
            Eye::Find(i) => self.finds[i].target.clone(),
            Eye::Piece(p) => Target::Showpiece(p),
            Eye::Body(b) => Target::Body(b),
        }
    }

    pub(super) fn eye_dir(&self, eye: Eye, now: UnixMs, hz: &Mat3, prec: &Mat3) -> Option<Vec3> {
        match eye {
            Eye::Find(i) => self.find_dir(i, now, hz, prec),
            Eye::Piece(p) => {
                let piece = &self.sky.lists.showpieces[p];
                Some(apply(hz, apply(prec, unit(piece.ra, piece.dec))))
            }
            Eye::Body(b) => {
                let s = see(b, self.observer, now);
                Some(from_alt_az(s.alt, s.az))
            }
        }
    }

    pub(super) fn eye_photo(&self, eye: Eye) -> Option<String> {
        match eye {
            Eye::Find(i) => self.photo_id(i),
            Eye::Piece(p) => Some(self.sky.lists.showpieces[p].id.clone()),
            Eye::Body(b) => self
                .photos
                .credit(b.id())
                .filter(|c| !c.card_only)
                .map(|_| b.id().to_owned()),
        }
    }

    /// Chooses the photograph for this frame and eases it in and out; the
    /// dots it replaces fade the other way.
    pub(super) fn update_eye(&mut self, now: UnixMs, hz: &Mat3, prec: &Mat3, dt: f64) {
        let want = self.eye_target(now, hz, prec);
        if let Some((i, _)) = want
            && self.eye != Some(i)
            && self.eye_alpha < 0.05
        {
            self.eye = Some(i);
        }
        let target = match want {
            Some((i, size)) if self.eye == Some(i) => size,
            _ => 0.0,
        };
        let k = 1.0 - (-dt / 0.3).exp();
        self.eye_alpha += (target - self.eye_alpha) * k;
    }

    /// The photograph to draw this frame, if any.
    pub(super) fn eyepiece(
        &mut self,
        now: UnixMs,
        hz: &Mat3,
        prec: &Mat3,
    ) -> Option<crate::view::Eyepiece> {
        if self.eye_alpha < 0.01 {
            return None;
        }
        let eye = self.eye?;
        let id = self.eye_photo(eye)?;
        let v = self.eye_dir(eye, now, hz, prec)?;
        let cam = self.camera;
        let (x, y) = cam.project(v)?;
        // Which way celestial north points on the screen, at the object.
        let pole = apply(hz, [0.0, 0.0, 1.0]);
        let along = dot3(pole, v);
        let t = [
            pole[0] - along * v[0],
            pole[1] - along * v[1],
            pole[2] - along * v[2],
        ];
        let n = dot3(t, t).sqrt().max(1e-9);
        let nudge = [
            v[0] + t[0] / n * 1e-5,
            v[1] + t[1] / n * 1e-5,
            v[2] + t[2] / n * 1e-5,
        ];
        let (nx, ny) = cam
            .project(nudge)
            .map_or((0.0, -1.0), |(a, b)| (a - x, b - y));
        let north = nx.atan2(-ny).to_degrees();
        let credit = self.photos.credit(&id).cloned()?;
        let rotation = north - credit.north.unwrap_or(0.0);
        let phase = match self.eye_of(eye) {
            // The Moon and the inner planets wear tonight's phase.
            Target::Body(body @ (Body::Moon | Body::Mercury | Body::Venus)) => {
                let seen = see(body, self.observer, now);
                let sun = see(Body::Sun, self.observer, now);
                let sv = from_alt_az(sun.alt, sun.az);
                let along = dot3(sv, v);
                let t = [
                    sv[0] - along * v[0],
                    sv[1] - along * v[1],
                    sv[2] - along * v[2],
                ];
                let n = dot3(t, t).sqrt().max(1e-9);
                let nudge = [
                    v[0] + t[0] / n * 1e-5,
                    v[1] + t[1] / n * 1e-5,
                    v[2] + t[2] / n * 1e-5,
                ];
                let (sx, sy) = cam
                    .project(nudge)
                    .map_or((1.0, 0.0), |(a, b)| (a - x, b - y));
                let l = (sx * sx + sy * sy).sqrt().max(1e-9);
                let (sx, sy) = (sx / l, sy / l);
                // Into the picture's own frame, before it is turned.
                let r = rotation.to_radians();
                let sun_in_picture = (sx * r.cos() + sy * r.sin(), -sx * r.sin() + sy * r.cos());
                let angle = if body == Body::Moon {
                    180.0 - seen.position.elongation
                } else {
                    seen.position
                        .phase
                        .mul_add(2.0, -1.0)
                        .clamp(-1.0, 1.0)
                        .acos()
                        .to_degrees()
                };
                Some(crate::eyepiece::Phase {
                    sun: sun_in_picture,
                    angle,
                })
            }
            _ => None,
        };
        let texture = self.photos.texture(&id, phase)?;
        let radius = self.target_half(&self.eye_of(eye), cam.fov);
        let aspect = texture.height() as f64 / texture.width().max(1) as f64;
        // The picture's middle, from the object, turned as the picture is.
        let [cx, cy] = credit.centre.unwrap_or([0.5, 0.5]);
        let (dx, dy) = (
            (0.5 - cx) * 2.0 * radius,
            (0.5 - cy) * 2.0 * radius * aspect,
        );
        let (s, c) = rotation.to_radians().sin_cos();
        let offset = (dx * c - dy * s, dx * s + dy * c);
        Some(crate::view::Eyepiece {
            texture,
            x,
            y,
            radius,
            aspect,
            offset,
            rotation,
            // Saturn's rings reach past a disc, so it's laid over the sky instead.
            disc: matches!(self.eye_of(eye), Target::Body(b) if b != Body::Saturn),
            credit: credit.credit.clone(),
            alpha: self.eye_alpha,
        })
    }

    /// A disc lit from the Sun's side: the Moon, or a planet close up.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lit_disc(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        v: Vec3,
        sun: Vec3,
        phase_angle: f64,
        color: Rgb,
        amount: f32,
        mottled: bool,
    ) {
        // Which way the Sun is, on the screen.
        let along = dot3(sun, v);
        let t = [
            sun[0] - along * v[0],
            sun[1] - along * v[1],
            sun[2] - along * v[2],
        ];
        let n = dot3(t, t).sqrt().max(1e-9);
        let nudge = [
            v[0] + t[0] / n * 0.01,
            v[1] + t[1] / n * 0.01,
            v[2] + t[2] / n * 0.01,
        ];
        let (ux, uy) = match (self.camera.project(v), self.camera.project(nudge)) {
            (Some(a), Some(b)) => {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let l = (dx * dx + dy * dy).sqrt().max(1e-9);
                (dx / l, dy / l)
            }
            _ => (1.0, 0.0),
        };
        let phase_angle = phase_angle.to_radians();
        let (si, ci) = (phase_angle.sin(), phase_angle.cos());
        let pitch = self.field.pitch as f64;
        let soft = (pitch / radius).min(0.5);
        self.field.disc(x, y, radius, |u, w| {
            let rho2 = u * u + w * w;
            if rho2 > 1.0 + soft {
                return None;
            }
            let edge = ((1.0 - rho2.sqrt()) / soft + 0.5).clamp(0.0, 1.0);
            let z = (1.0 - rho2.min(1.0)).sqrt();
            let lit = (u * ux + w * uy) * si + z * ci;
            let day = smoothstep(lit * 6.0 + 0.5);
            let texture = if mottled {
                let n = noise3(u * 3.1 + 7.0, w * 3.1 + 3.0, 1.7) * 0.6
                    + noise3(u * 7.3, w * 7.3, 4.2) * 0.4;
                0.5 + 0.62 * n
            } else {
                1.0
            };
            let limb = 0.8 + 0.2 * z;
            let value = (day * texture * limb) as f32 * amount + (1.0 - day as f32) * 0.03;
            Some((color, value * edge as f32))
        });
    }
}

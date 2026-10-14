//! Программная отрисовка утверждённой геометрии assets/rings.js в XRGB.
//! Три каменные ленты, 192 сектора × 8 граней, глубина и 48 независимых рун.
//! Кадры вычисляются во время выполнения: конечной повторяющейся петли нет.

use std::f32::consts::TAU;
use std::io::{self, Read};

const RADII: [f32; 3] = [1.0, 1.2672599504, 1.5339610719];
const PERIODS: [f32; 3] = [12.0, 17.0, 23.0];
type V = [f32; 3];
type M = [V; 3];
fn add(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] + b[i])
}
fn mul(a: V, n: f32) -> V {
    a.map(|v| v * n)
}
fn dot(a: V, b: V) -> f32 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V) -> V {
    mul(a, 1.0 / dot(a, a).sqrt().max(0.00001))
}
fn transform(m: M, p: V) -> V {
    add(add(mul(m[0], p[0]), mul(m[1], p[1])), mul(m[2], p[2]))
}
fn inverse(m: M, p: V) -> V {
    m.map(|v| dot(v, p))
}
fn local(a: f32, r: f32, z: f32) -> V {
    [a.cos() * r, a.sin() * r, z]
}
fn matrix(k: usize, t: f32) -> M {
    let n = norm(
        [
            [0.0, 0.93, 0.367],
            [0.86, -0.2, 0.47],
            [-0.72, -0.27, 0.639],
        ][k],
    );
    let u = norm(cross([0.0, 1.0, 0.0], n));
    let v = cross(n, u);
    let axis = norm([[0.27, 0.68, 1.0], [0.9, -0.28, 0.72], [-0.58, 0.85, 0.63]][k]);
    let (sn, cs) = ((t - 2.4) * TAU / PERIODS[k]).sin_cos();
    [u, v, n].map(|p| {
        add(
            add(mul(p, cs), mul(cross(axis, p), sn)),
            mul(axis, dot(axis, p) * (1.0 - cs)),
        )
    })
}

struct Random(u64);
impl Random {
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn period(&mut self) -> f64 {
        5.0 + self.unit() * 5.0
    }
}
#[derive(Clone, Copy)]
struct Pulse {
    start: f64,
    duration: f64,
}
impl Pulse {
    fn sample(&mut self, time: f64, random: &mut Random) -> f32 {
        while time >= self.start + self.duration {
            self.start += self.duration;
            self.duration = random.period();
        }
        // На стыке значение и первая производная равны нулю при любом периоде.
        (0.5 - 0.5 * ((time - self.start) / self.duration * std::f64::consts::TAU).cos()) as f32
    }
}
#[derive(Clone, Copy)]
struct Vertex {
    position: V,
    normal: V,
}
struct Triangle {
    vertices: [Vertex; 3],
    color: V,
    emission: u8,
    rune: usize,
    ring: usize,
}
struct Texture {
    width: usize,
    height: usize,
    bytes: Vec<u8>,
    channels: usize,
}
impl Texture {
    fn load(path: &std::path::Path) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(io::Error::other)?;
        let mut bytes = vec![
            0;
            reader
                .output_buffer_size()
                .ok_or_else(|| io::Error::other("invalid texture size"))?
        ];
        let info = reader.next_frame(&mut bytes).map_err(io::Error::other)?;
        let channels = match info.color_type {
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            _ => return Err(io::Error::other("unsupported stone texture")),
        };
        bytes.truncate(info.buffer_size());
        Ok(Self {
            width: info.width as usize,
            height: info.height as usize,
            bytes,
            channels,
        })
    }
    fn sample(&self, u: f32, v: f32) -> f32 {
        let uv = |f: f32| 1.0 - (f.rem_euclid(2.0) - 1.0).abs();
        let x = uv(u) * (self.width - 1) as f32;
        let y = uv(v) * (self.height - 1) as f32;
        let (ix, iy) = (x as usize, y as usize);
        let pixel =
            |a: usize, b: usize| self.bytes[(b * self.width + a) * self.channels] as f32 / 255.0;
        let (fx, fy) = (x.fract(), y.fract());
        let a = pixel(ix, iy) * (1.0 - fx) + pixel((ix + 1).min(self.width - 1), iy) * fx;
        let b = pixel(ix, (iy + 1).min(self.height - 1)) * (1.0 - fx)
            + pixel((ix + 1).min(self.width - 1), (iy + 1).min(self.height - 1)) * fx;
        a * (1.0 - fy) + b * fy
    }
    fn tone(&self, p: V, n: V) -> f32 {
        let weights = norm(n).map(|v| v.abs().powi(4));
        let sum = weights.into_iter().sum::<f32>().max(0.001);
        let p = add(mul(p, 0.72), [0.3, 0.5, 0.1]);
        dot(
            weights,
            [
                self.sample(p[1], p[2]),
                self.sample(p[0], p[2]),
                self.sample(p[0], p[1]),
            ],
        ) / sum
    }
}

pub(crate) struct Rings {
    mesh: Vec<Triangle>,
    texture: Texture,
    pulses: [Pulse; 48],
    random: Random,
}
impl Rings {
    pub(crate) fn load_default() -> io::Result<Self> {
        Self::load_from(std::path::Path::new(
            "/resources/images/stone-reference.png",
        ))
    }
    pub(crate) fn load_from(path: &std::path::Path) -> io::Result<Self> {
        // Начальная энтропия ядра: повторный вход/запуск не воспроизводит старую петлю.
        let mut seed = [0; 8];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut seed)?;
        let mut random = Random(u64::from_ne_bytes(seed).max(1));
        let pulses = std::array::from_fn(|_| {
            let duration = random.period();
            Pulse {
                start: -random.unit() * duration,
                duration,
            }
        });
        Ok(Self {
            mesh: mesh(),
            texture: Texture::load(path)?,
            pulses,
            random,
        })
    }
    pub(crate) fn draw(
        &mut self,
        pixels: &mut [u8],
        pitch: usize,
        width: u32,
        height: u32,
        t: f64,
        reduced: bool,
    ) {
        let size = 350
            .min(width as usize)
            .min(height.saturating_sub(120) as usize);
        if size < 16 {
            return;
        }
        let x0 = (width as usize - size) / 2;
        let y0 = (height as usize).saturating_sub(size) / 2;
        let y0 = y0.saturating_sub(65);
        let matrices: [M; 3] =
            std::array::from_fn(|k| matrix(k, if reduced { 2.4 } else { t as f32 }));
        let glows: [f32; 48] = std::array::from_fn(|i| {
            if reduced {
                0.5 - 0.5 * (((i % 16 * 7 + i / 16 * 3) % 16) as f32 / 16.0 * TAU).cos()
            } else {
                self.pulses[i].sample(t, &mut self.random)
            }
        });
        let mut depth = vec![f32::NEG_INFINITY; size * size];
        // Все непрозрачные грани записывают глубину раньше рун. Ореолы только читают её.
        for emission in [false, true] {
            for triangle in self.mesh.iter().filter(|a| (a.emission > 0) == emission) {
                raster(
                    triangle,
                    matrices,
                    &glows,
                    &self.texture,
                    &mut depth,
                    pixels,
                    pitch,
                    size,
                    x0,
                    y0,
                );
            }
        }
    }
}

fn mesh() -> Vec<Triangle> {
    let mut out = Vec::new();
    // Восемь знаков перенесены из rings.js без замены экранным шрифтом.
    let glyphs: Vec<Vec<Vec<[f32; 2]>>> = vec![
        vec![
            vec![[-0.7, -0.5], [0., -1.], [0.7, -0.5], [0., 0.], [-0.7, -0.5]],
            vec![[0., -1.], [0., 1.]],
            vec![[0., 0.25], [0.65, 0.7], [0., 1.]],
        ],
        vec![
            vec![[0., -1.], [0., 1.]],
            vec![[-0.65, -0.65], [0.65, -0.1], [-0.65, 0.4]],
            vec![[0., 0.4], [0.6, 0.85]],
        ],
        vec![
            vec![[0., -1.], [-0.75, 0.], [0., 0.8], [0.75, 0.], [0., -1.]],
            vec![[0., 0.8], [0., 1.1]],
        ],
        vec![
            vec![[-0.6, -1.], [-0.6, 1.]],
            vec![[-0.6, -0.7], [0.7, -0.15], [-0.6, 0.35]],
            vec![[-0.6, 0.35], [0.65, 0.85]],
        ],
        vec![
            vec![[-0.7, -0.85], [0.7, 0.85]],
            vec![[-0.7, 0.85], [0.7, -0.85]],
            vec![[0., -1.05], [0., 1.05]],
        ],
        vec![
            vec![
                [-0.6, -0.8],
                [0.5, -0.8],
                [0.65, 0.],
                [0., 0.55],
                [-0.65, 0.],
                [-0.6, -0.8],
            ],
            vec![[0., 0.55], [0., 1.]],
        ],
        vec![
            vec![
                [-0.65, -1.],
                [-0.65, 0.2],
                [0., 0.85],
                [0.65, 0.2],
                [0.65, -1.],
            ],
            vec![[0., -0.5], [0., 1.1]],
        ],
        vec![
            vec![[0., -1.], [-0.7, -0.15], [0.7, 0.25], [0., 1.]],
            vec![[-0.65, 0.9], [0.65, -0.9]],
        ],
    ];
    for (k, radius) in RADII.into_iter().enumerate() {
        let (outer, inner, h, b) = (radius + 0.055, radius - 0.055, 0.135, 0.014);
        let section = [
            [outer, -h + b],
            [outer, h - b],
            [outer - b, h],
            [inner + b, h],
            [inner, h - b],
            [inner, -h + b],
            [inner + b, -h],
            [outer - b, -h],
        ];
        for i in 0..192 {
            let (a, b) = (i as f32 * TAU / 192., (i + 1) as f32 * TAU / 192.);
            for j in 0..8 {
                let (s, e) = (section[j], section[(j + 1) % 8]);
                let (dr, dz) = (e[0] - s[0], e[1] - s[1]);
                let len = dr.hypot(dz);
                let at = |a: f32, r, z| Vertex {
                    position: local(a, r, z),
                    normal: [a.cos() * dz / len, a.sin() * dz / len, -dr / len],
                };
                let pts = [
                    at(a, s[0], s[1]),
                    at(b, s[0], s[1]),
                    at(b, e[0], e[1]),
                    at(a, e[0], e[1]),
                ];
                let color = match j {
                    0 => [0.62, 0.65, 0.68],
                    4 => [0.51, 0.54, 0.57],
                    2 | 6 => [0.8, 0.82, 0.83],
                    _ => [0.73, 0.76, 0.78],
                };
                quad(&mut out, pts, color, 0, 0, k);
            }
        }
        for i in 0..16 {
            let a = (i as f32 * 12. + 3.5) * TAU / 192.;
            let normal = [a.cos(), a.sin(), 0.];
            let point = |p: [f32; 2]| Vertex {
                position: local(a + p[0] * 0.054 / outer, outer + 0.005, p[1] * 0.096),
                normal,
            };
            for path in &glyphs[(i * 3 + k * 2) % 8] {
                for segment in path.windows(2) {
                    let (p, q) = (segment[0], segment[1]);
                    let (dx, dy) = ((q[0] - p[0]) * 0.054, (q[1] - p[1]) * 0.096);
                    let len = dx.hypot(dy);
                    for (w, emission, color) in [
                        (0.044, 3, [0.2, 0.85, 0.8]),
                        (0.025, 2, [0.25, 0.9, 0.85]),
                        (0.012, 1, [0.65, 1., 0.94]),
                    ] {
                        let (ox, oy) = (-dy / len * w / 0.054, dx / len * w / 0.096);
                        let pts = [
                            [p[0] + ox, p[1] + oy],
                            [q[0] + ox, q[1] + oy],
                            [q[0] - ox, q[1] - oy],
                            [p[0] - ox, p[1] - oy],
                        ]
                        .map(point);
                        quad(&mut out, pts, color, emission, i, k);
                    }
                }
            }
        }
    }
    out
}
fn quad(out: &mut Vec<Triangle>, p: [Vertex; 4], color: V, emission: u8, rune: usize, ring: usize) {
    for vertices in [[p[0], p[1], p[2]], [p[0], p[2], p[3]]] {
        out.push(Triangle {
            vertices,
            color,
            emission,
            rune,
            ring,
        });
    }
}

fn occludes(p: V, d: V, radius: f32) -> f32 {
    let a = d[0] * d[0] + d[1] * d[1];
    let b = p[0] * d[0] + p[1] * d[1];
    let outer = radius + 0.055;
    let inner = radius - 0.055;
    if a < 0.00001 {
        return 0.;
    }
    let disc = b * b - a * (p[0] * p[0] + p[1] * p[1] - outer * outer);
    if disc < 0. {
        return 0.;
    }
    let mut lo = 0.015f32.max((-b - disc.sqrt()) / a);
    let mut hi = (-b + disc.sqrt()) / a;
    if d[2].abs() < 0.00001 {
        if p[2].abs() > 0.135 {
            return 0.;
        }
    } else {
        let (z0, z1) = ((-0.135 - p[2]) / d[2], (0.135 - p[2]) / d[2]);
        lo = lo.max(z0.min(z1));
        hi = hi.min(z0.max(z1));
    }
    if hi <= lo {
        return 0.;
    }
    let hole = b * b - a * (p[0] * p[0] + p[1] * p[1] - inner * inner);
    if hole < 0. {
        return 1.;
    }
    if lo < (-b - hole.sqrt()) / a || hi > (-b + hole.sqrt()) / a {
        1.
    } else {
        0.
    }
}

#[allow(clippy::too_many_arguments)]
fn raster(
    tri: &Triangle,
    matrices: [M; 3],
    glows: &[f32; 48],
    texture: &Texture,
    depth: &mut [f32],
    pixels: &mut [u8],
    pitch: usize,
    size: usize,
    x0: usize,
    y0: usize,
) {
    let m = matrices[tri.ring];
    let world = tri.vertices.map(|v| transform(m, v.position));
    let scale = size as f32 * 0.276;
    let screen = world.map(|p| {
        [
            size as f32 / 2. + p[0] * scale,
            size as f32 / 2. + p[1] * scale,
            p[2],
        ]
    });
    let edge = |a: V, b: V, x: f32, y: f32| (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
    let area = edge(screen[0], screen[1], screen[2][0], screen[2][1]);
    if area.abs() < 0.0001 {
        return;
    }
    let min = |axis| {
        screen
            .iter()
            .map(|p| p[axis])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as usize
    };
    let max = |axis| {
        (screen
            .iter()
            .map(|p| p[axis])
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .max(0.) as usize)
            .min(size)
    };
    let pulse = glows[tri.ring * 16 + tri.rune];
    for y in min(1)..max(1) {
        for x in min(0)..max(0) {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w = [
                edge(screen[1], screen[2], px, py) / area,
                edge(screen[2], screen[0], px, py) / area,
                edge(screen[0], screen[1], px, py) / area,
            ];
            // Полуоткрытые грани: общий край двух треугольников не удваивает свечение.
            let top_left = |a: V, b: V| {
                let (a, b) = if area < 0. { (b, a) } else { (a, b) };
                b[1] > a[1] || (b[1] == a[1] && b[0] < a[0])
            };
            if w.iter().any(|v| *v < 0.)
                || (w[0] == 0. && !top_left(screen[1], screen[2]))
                || (w[1] == 0. && !top_left(screen[2], screen[0]))
                || (w[2] == 0. && !top_left(screen[0], screen[1]))
            {
                continue;
            }
            let interpolate =
                |v: [V; 3]| add(add(mul(v[0], w[0]), mul(v[1], w[1])), mul(v[2], w[2]));
            let p = interpolate(world);
            let index = y * size + x;
            if p[2] + 0.00001 < depth[index] {
                continue;
            }
            let local_n = interpolate(tri.vertices.map(|v| v.normal));
            let n = norm(transform(m, local_n));
            let (color, alpha) = if tri.emission > 0 {
                let facing = (n[2] / 0.2).clamp(0., 1.);
                let facing = facing * facing * (3. - 2. * facing);
                if facing <= 0.001 {
                    continue;
                }
                match tri.emission {
                    3 => (tri.color, 0.12 * pulse * facing),
                    2 => (tri.color, 0.30 * pulse * facing),
                    _ => (
                        add(mul([0.57, 0.60, 0.63], 1. - pulse), mul(tri.color, pulse)),
                        (0.015 + 0.965 * pulse) * facing,
                    ),
                }
            } else {
                let ray = norm([-0.4, -0.65, 1.]);
                let key = dot(n, ray).max(0.);
                let fill = dot(n, norm([0.8, 0.3, 0.2])).max(0.);
                let pos = add(p, mul(n, 0.006));
                let shadow = [
                    ray,
                    norm(add(ray, [0.045, 0.025, 0.])),
                    norm(add(ray, [-0.045, -0.025, 0.])),
                ]
                .into_iter()
                .map(|d| {
                    (0..3)
                        .filter(|k| *k != tri.ring)
                        .map(|k| {
                            occludes(inverse(matrices[k], pos), inverse(matrices[k], d), RADII[k])
                        })
                        .fold(0., f32::max)
                })
                .sum::<f32>()
                    / 3.;
                let light = 0.48 + 0.48 * key * (1. - 0.70 * shadow) + 0.15 * fill;
                let tone = 0.62
                    + 0.72 * texture.tone(interpolate(tri.vertices.map(|v| v.position)), local_n);
                let color = mul(tri.color, light * tone);
                depth[index] = p[2];
                ([color[0], color[1] * 0.998, color[2] * 0.985], 1.)
            };
            let offset = (y + y0) * pitch + (x + x0) * 4;
            if let Some(pixel) = pixels.get_mut(offset..offset + 4) {
                for c in 0..3 {
                    pixel[2 - c] = ((pixel[2 - c] as f32 * (1. - alpha)
                        + color[c].clamp(0., 1.) * 255. * alpha)
                        .round()) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotations_close_after_their_period_and_preserve_length() {
        for k in 0..3 {
            let a = matrix(k, 2.4);
            let b = matrix(k, 2.4 + PERIODS[k]);
            for i in 0..3 {
                assert!((dot(a[i], a[i]) - 1.).abs() < 1e-5);
                for j in 0..3 {
                    assert!((a[i][j] - b[i][j]).abs() < 1e-5);
                }
            }
        }
    }
    #[test]
    fn random_cycles_are_continuous_and_choose_new_periods() {
        let mut random = Random(12345);
        let mut pulse = Pulse {
            start: -1.3,
            duration: 7.1,
        };
        let boundary = pulse.start + pulse.duration;
        let old = pulse.duration;
        assert!(pulse.sample(boundary - 0.0001, &mut random) < 1e-6);
        assert!(pulse.sample(boundary + 0.0001, &mut random) < 1e-6);
        assert!((5.0..10.0).contains(&pulse.duration));
        assert_ne!(old, pulse.duration);
        for _ in 0..100 {
            let boundary = pulse.start + pulse.duration;
            assert!(pulse.sample(boundary, &mut random) < 1e-6);
            assert!((5.0..10.0).contains(&pulse.duration));
        }
    }
    #[test]
    fn geometry_has_three_separate_shells_and_48_runes() {
        let mesh = mesh();
        let mut runes = [false; 48];
        for t in mesh {
            for v in t.vertices {
                let r = dot(v.position, v.position).sqrt();
                assert!(r >= RADII[t.ring] - 0.055 - 1e-5);
                if t.emission == 1 {
                    runes[t.ring * 16 + t.rune] = true;
                }
            }
        }
        assert!(runes.into_iter().all(|v| v));
        for k in 0..2 {
            assert!((RADII[k] + 0.060).hypot(0.19) < RADII[k + 1] - 0.055);
        }
    }
}

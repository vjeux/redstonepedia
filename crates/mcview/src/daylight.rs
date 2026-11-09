//! The game's sky and light through the day (vjeux 2026-11-08: "Also can we get a visual
//! indication for the day night cycle since it's important for how the farm works"): the
//! overworld's environment attributes at a day time, sampled the way 26.3's
//! `EnvironmentAttributeSystem` does. Its layers in its order: the dimension's values
//! (`dimension_type/overworld.json`: the plains' biome gives the same sky and fog), the `day`
//! timeline's tracks (`timeline/day.json`, kept below: `KeyframeTrackSampler` over a 24000-tick
//! period, each track's ease and modifier), then rain and thunder (`WeatherAttributes`, each
//! blended in by its level). What the view does with them is the client's: the lightmap
//! (`lightmap.fsh`: the sky light factor and colour, the ambient colour, the block light's tint),
//! the sky's colour, the horizon's fog colour (`AtmosphericFogEnvironment.getBaseColor`), the sun
//! and the moon at their angles (`SkyRenderer`).

/// An RGB colour as the game's attributes keep it (`ARGB.vector3fFromRGB24`).
const fn rgb(c: u32) -> [f32; 3] {
    [((c >> 16) & 0xff) as f32 / 255.0, ((c >> 8) & 0xff) as f32 / 255.0, (c & 0xff) as f32 / 255.0]
}

/// An ARGB colour (`ARGB.vector4fFromARGB32`): r, g, b, a.
const fn argb(c: u32) -> [f32; 4] {
    [((c >> 16) & 0xff) as f32 / 255.0, ((c >> 8) & 0xff) as f32 / 255.0, (c & 0xff) as f32 / 255.0, ((c >> 24) & 0xff) as f32 / 255.0]
}

const WHITE: [f32; 3] = rgb(0xffffff);
const BLACK: [f32; 3] = rgb(0x000000);

/// The overworld's own values (`dimension_type/overworld.json`'s attributes, else each
/// attribute's default: `EnvironmentAttributes`).
pub const SKY_COLOR: [f32; 3] = rgb(0x78a7ff);
pub const FOG_COLOR: [f32; 3] = rgb(0xc0d8ff);
pub const AMBIENT_LIGHT_COLOR: [f32; 3] = rgb(0x0a0a0a);
/// `visual/block_light_tint`'s default (-10100: 0xffd88c).
pub const BLOCK_LIGHT_TINT: [f32; 3] = rgb(0xffd88c);
/// `Timelines.NIGHT_SKY_LIGHT_COLOR` (`ARGB.colorFromFloat(1, 0.48, 0.48, 1)`).
const NIGHT_SKY_LIGHT_COLOR: [f32; 3] = rgb(0x7a7aff);

/// The day timeline's tracks (`timeline/day.json`), ticks of the day and values.
const SKY_COLOR_TRACK: [(i64, [f32; 3]); 4] = [(133, WHITE), (11867, WHITE), (13670, BLACK), (22330, BLACK)];
const FOG_COLOR_TRACK: [(i64, [f32; 3]); 4] = [(133, WHITE), (11867, WHITE), (13670, rgb(0x0f0f16)), (22330, rgb(0x0f0f16))];
const SKY_LIGHT_COLOR_TRACK: [(i64, [f32; 3]); 4] = [(730, WHITE), (11270, WHITE), (13140, NIGHT_SKY_LIGHT_COLOR), (22860, NIGHT_SKY_LIGHT_COLOR)];
const SKY_LIGHT_FACTOR_TRACK: [(i64, f32); 4] = [(730, 1.0), (11270, 1.0), (13140, 0.24), (22860, 0.24)];
const STAR_BRIGHTNESS_TRACK: [(i64, f32); 12] = [
    (92, 0.037),
    (627, 0.0),
    (11373, 0.0),
    (11732, 0.016),
    (11959, 0.044),
    (12399, 0.143),
    (12729, 0.258),
    (13228, 0.5),
    (22772, 0.5),
    (23032, 0.364),
    (23356, 0.225),
    (23758, 0.101),
];
const SUNRISE_SUNSET_TRACK: [(i64, u32); 32] = [
    (71, 0x5fefa333),
    (310, 0x29f5ba33),
    (565, 0x06fbd433),
    (730, 0x00ffe533),
    (11270, 0x00ffe533),
    (11397, 0x04fcd833),
    (11522, 0x0ff9cb33),
    (11690, 0x29f5ba33),
    (11929, 0x5fefa333),
    (12243, 0xb1e78733),
    (12358, 0xcce47e33),
    (12512, 0xe9e07233),
    (12613, 0xf6dd6b33),
    (12732, 0xfeda6333),
    (12841, 0xfed75c33),
    (13035, 0xecd25133),
    (13252, 0xc1cc4733),
    (13775, 0x36be3733),
    (13888, 0x1fbb3533),
    (14039, 0x09b73333),
    (14192, 0x00b33333),
    (21807, 0x00b23333),
    (21961, 0x09b73333),
    (22112, 0x1fbb3533),
    (22225, 0x36be3733),
    (22748, 0xc1cc4733),
    (22965, 0xecd25133),
    (23159, 0xfed75c33),
    (23272, 0xfeda6333),
    (23488, 0xe9e07233),
    (23642, 0xcce47e33),
    (23757, 0xb1e78733),
];
/// The sun's, moon's and stars' ease (`cubic_bezier [0.362, 0.241, 0.638, 0.759]`).
const SKY_ANGLE_EASE: [f32; 4] = [0.362, 0.241, 0.638, 0.759];
const PERIOD: i64 = 24000;

/// The environment at a moment: what the sky and the lightmap read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    pub sky_color: [f32; 3],
    pub fog_color: [f32; 3],
    pub sky_light_color: [f32; 3],
    pub sky_light_factor: f32,
    pub ambient: [f32; 3],
    pub block_tint: [f32; 3],
    /// Degrees (0: the sun at noon's zenith).
    pub sun_angle: f32,
    pub moon_angle: f32,
    pub star_brightness: f32,
    /// r, g, b, a.
    pub sunrise: [f32; 4],
    /// The weather's levels (0..1): the client darkens the fog's sky colour with them too.
    pub rain: f32,
    pub thunder: f32,
}

impl Env {
    /// Noon in clear weather (the pictures' light: what the view drew before the day cycle).
    pub fn noon() -> Env {
        at(6000, 0.0, 0.0)
    }

    /// The sun is up (the sky light factor's day plateau and its dusk half): the clock's sun
    /// icon, else its moon.
    pub fn is_day(&self) -> bool {
        self.sky_light_factor >= 0.62
    }
}

/// The day time the sky light factor falls through the middle of its dusk (11270 -> 13140: 1 to
/// 0.24) and rises through it at dawn (22860 -> 24730): the view's night (the scrubber's dark
/// spans, the clock's moon) runs from [`DUSK`] to [`DAWN`] the next day.
pub const DUSK: i64 = 12205;
pub const DAWN: i64 = -205;

/// `EasingType.CubicBezier.apply`: x solved for t by Newton-Raphson (4 steps, each at most 0.25,
/// stopping under 1e-5), else bisection, then y at t.
fn cubic_bezier(c: [f32; 4], x: f32) -> f32 {
    let curve = |v1: f32, v2: f32| (3.0 * v1 - 3.0 * v2 + 1.0, -6.0 * v1 + 3.0 * v2, 3.0 * v1);
    let (xa, xb, xc) = curve(c[0], c[2]);
    let (ya, yb, yc) = curve(c[1], c[3]);
    let sample = |a: f32, b: f32, cc: f32, t: f32| ((a * t + b) * t + cc) * t;
    let gradient = |a: f32, b: f32, cc: f32, t: f32| (3.0 * a * t + 2.0 * b) * t + cc;
    let solve = || -> f32 {
        let mut t = x;
        for _ in 0..4 {
            let error = sample(xa, xb, xc, t) - x;
            if error.abs() < 1e-5 {
                return t;
            }
            let g = gradient(xa, xb, xc, t);
            if g < 1e-5 {
                break;
            }
            t -= (error / g).clamp(-0.25, 0.25);
        }
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        while t0 < t1 {
            let error = sample(xa, xb, xc, t) - x;
            if error.abs() < 1e-5 {
                return t;
            }
            if error < 0.0 {
                t0 = t;
            } else {
                t1 = t;
            }
            let next = (t1 + t0) / 2.0;
            if next == t {
                break;
            }
            t = next;
        }
        t
    };
    sample(ya, yb, yc, solve())
}

/// `KeyframeTrackSampler.sample` of a periodic track at day time `t`: the segment around it (the
/// last keyframe wraps to the first), its alpha eased, the values lerped.
fn sample<T: Copy>(keys: &[(i64, T)], t: i64, ease: Option<[f32; 4]>, lerp: impl Fn(f32, T, T) -> T) -> T {
    let t = t.rem_euclid(PERIOD);
    let (first, last) = (keys[0], keys[keys.len() - 1]);
    let mut segments: Vec<((i64, T), (i64, T))> = vec![((last.0 - PERIOD, last.1), first)];
    for w in keys.windows(2) {
        segments.push((w[0], w[1]));
    }
    segments.push((last, (first.0 + PERIOD, first.1)));
    let (from, to) = segments.iter().copied().find(|(_, to)| t < to.0).unwrap_or(*segments.last().unwrap());
    if t <= from.0 {
        return from.1;
    }
    if t >= to.0 {
        return to.1;
    }
    let alpha = (t - from.0) as f32 / (to.0 - from.0) as f32;
    let alpha = ease.map(|c| cubic_bezier(c, alpha)).unwrap_or(alpha);
    lerp(alpha, from.1, to.1)
}

fn lerp(a: f32, x: f32, y: f32) -> f32 {
    x + a * (y - x)
}

fn lerp3(a: f32, x: [f32; 3], y: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| lerp(a, x[k], y[k]))
}

fn lerp4(a: f32, x: [f32; 4], y: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|k| lerp(a, x[k], y[k]))
}

fn mul3(x: [f32; 3], y: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| x[k] * y[k])
}

fn mul4(x: [f32; 4], y: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|k| x[k] * y[k])
}

/// `ColorModifier.BLEND_TO_GRAY_RGB`: toward the colour's grey (0.3 r + 0.59 g + 0.11 b) times
/// `brightness`, by `factor`.
fn blend_to_gray(c: [f32; 3], brightness: f32, factor: f32) -> [f32; 3] {
    let g = (c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11) * brightness;
    lerp3(factor, c, [g, g, g])
}

/// The overworld's environment at day time `day_time` with rain and thunder at `rain` and
/// `thunder` (0..1, the level's `getRainLevel` / `getThunderLevel`).
pub fn at(day_time: i64, rain: f32, thunder: f32) -> Env {
    let t = day_time;
    // the dimension, then the day timeline (multiply; star brightness: maximum; the sunrise colour
    // and the angles: set)
    let mut sky_color = mul3(SKY_COLOR, sample(&SKY_COLOR_TRACK, t, None, lerp3));
    let mut fog_color = mul3(FOG_COLOR, sample(&FOG_COLOR_TRACK, t, None, lerp3));
    let mut sky_light_color = mul3(WHITE, sample(&SKY_LIGHT_COLOR_TRACK, t, None, lerp3));
    let mut sky_light_factor = sample(&SKY_LIGHT_FACTOR_TRACK, t, None, lerp);
    let mut star_brightness = sample(&STAR_BRIGHTNESS_TRACK, t, None, lerp).max(0.0);
    let sunrise_keys: Vec<(i64, [f32; 4])> = SUNRISE_SUNSET_TRACK.iter().map(|(k, c)| (*k, argb(*c))).collect();
    let mut sunrise = sample(&sunrise_keys, t, None, lerp4);
    let sun_angle = sample(&[(6000, 360.0), (6000, 0.0)], t, Some(SKY_ANGLE_EASE), lerp);
    let moon_angle = sample(&[(6000, 540.0), (6000, 180.0)], t, Some(SKY_ANGLE_EASE), lerp);
    // the weather (`WeatherAttributes`): rain at its level less thunder's, then thunder (their
    // colours through `ARGB.colorFromFloat`, each channel floored to 8 bits)
    let thunder = thunder.clamp(0.0, 1.0);
    let rain = rain.clamp(0.0, 1.0);
    for (level, gray, fog, light_alpha, sunrise_mul) in [(rain - thunder, (0.6, 0.75), rgb(0x7f7f99), 79.0 / 255.0, argb(0xff7f_7f99)), (thunder, (0.24, 0.94), rgb(0x3f3f4c), 134.0 / 255.0, argb(0xff3f_3f4c))] {
        if level <= 0.0 {
            continue;
        }
        let factor_alpha = if gray.0 == 0.6 { 0.3125 } else { 0.527_343_75 };
        sky_color = lerp3(level, sky_color, blend_to_gray(sky_color, gray.0, gray.1));
        fog_color = lerp3(level, fog_color, mul3(fog_color, fog));
        sky_light_color = lerp3(level, sky_light_color, lerp3(light_alpha, sky_light_color, NIGHT_SKY_LIGHT_COLOR));
        sky_light_factor = lerp(level, sky_light_factor, lerp(factor_alpha, sky_light_factor, 0.24));
        star_brightness = lerp(level, star_brightness, 0.0);
        sunrise = lerp4(level, sunrise, mul4(sunrise, sunrise_mul));
    }
    Env { sky_color, fog_color, sky_light_color, sky_light_factor, ambient: AMBIENT_LIGHT_COLOR, block_tint: BLOCK_LIGHT_TINT, sun_angle, moon_angle, star_brightness, sunrise, rain, thunder }
}

/// `AtmosphericFogEnvironment.getBaseColor`: the horizon's colour for a camera looking along
/// `forward` with a render distance of `chunks`: the fog colour, toward the sunrise colour when
/// looking at the sun's side, then toward the sky colour (darkened by the weather) by the render
/// distance's share.
pub fn horizon(env: &Env, forward: [f32; 3], chunks: f32) -> [f32; 3] {
    let mut fog = env.fog_color;
    if chunks >= 4.0 {
        let sun_x = if env.sun_angle.to_radians().sin() > 0.0 { -1.0 } else { 1.0 };
        let looking = forward[0] * sun_x;
        if looking > 0.0 && env.sunrise[3] > 0.0 {
            fog = lerp3(looking * env.sunrise[3], fog, [env.sunrise[0], env.sunrise[1], env.sunrise[2]]);
        }
    }
    let mut sky = env.sky_color;
    if env.rain > 0.0 {
        let (m, b) = (1.0 - env.rain * 0.5, 1.0 - env.rain * 0.4);
        sky = [sky[0] * m, sky[1] * m, sky[2] * b];
    }
    if env.thunder > 0.0 {
        let m = 1.0 - env.thunder * 0.5;
        sky = sky.map(|v| v * m);
    }
    let sky_fog_end = (512.0f32 / 16.0).min(chunks);
    let f = sky_fog_end / 32.0;
    let mix = if f < 0.0 { 0.25 } else if f > 1.0 { 1.0 } else { lerp(f, 0.25, 1.0) };
    let mix = 1.0 - mix.powf(0.25);
    lerp3(mix, fog, sky)
}

/// The lightmap's colour at (sky, block) light levels in `env`: its 16 x 16 texels (`lightmap.fsh`
/// with `LightmapRenderStateExtractor`'s inputs: the block factor 1.4 without its flicker, the
/// default brightness 0.5, no night vision, darkness or boss fog), fractional levels (a smooth-lit
/// corner's mean) between texels like the game's linear sampler (`light::lightmap` at noon).
pub fn lightmap(env: &Env, sky: f32, block: f32) -> [f32; 3] {
    let (s, b) = (sky.clamp(0.0, 15.0), block.clamp(0.0, 15.0));
    let (s0, b0) = (s.floor(), b.floor());
    let (s1, b1) = ((s0 + 1.0).min(15.0), (b0 + 1.0).min(15.0));
    let (fs, fb) = (s - s0, b - b0);
    let (t00, t01, t10, t11) = (texel(env, s0, b0), texel(env, s0, b1), texel(env, s1, b0), texel(env, s1, b1));
    std::array::from_fn(|k| {
        let lo = t00[k] + (t01[k] - t00[k]) * fb;
        let hi = t10[k] + (t11[k] - t10[k]) * fb;
        lo + (hi - lo) * fs
    })
}

/// One texel of the lightmap at whole levels.
fn texel(env: &Env, sky: f32, block: f32) -> [f32; 3] {
    let gb = |l: f32| l / (4.0 - 3.0 * l);
    let (bl, sl) = (block / 15.0, sky / 15.0);
    let (block_b, sky_b) = (gb(bl) * 1.4, gb(sl) * env.sky_light_factor);
    let pm = (2.0 * bl - 1.0) * (2.0 * bl - 1.0);
    let mut c = [0f32; 3];
    for k in 0..3 {
        let blc = env.block_tint[k] + (1.0 - env.block_tint[k]) * 0.9 * pm;
        c[k] = (env.ambient[k] + env.sky_light_color[k] * sky_b + blc * block_b).clamp(0.0, 1.0);
    }
    let max = c[0].max(c[1]).max(c[2]);
    if max <= 0.0 {
        return c;
    }
    let inv = 1.0 - max;
    let scaled = 1.0 - inv * inv * inv * inv;
    c.map(|v| v + (v * scaled / max - v) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_the_dimensions_light_and_midnight_the_nights() {
        let noon = Env::noon();
        assert_eq!(noon.sky_color, SKY_COLOR);
        assert_eq!(noon.fog_color, FOG_COLOR);
        assert_eq!(noon.sky_light_factor, 1.0);
        assert_eq!(noon.sky_light_color, WHITE);
        assert_eq!(noon.sun_angle, 0.0);
        assert!(noon.is_day());
        let midnight = at(18000, 0.0, 0.0);
        assert_eq!(midnight.sky_color, BLACK);
        assert!((midnight.sky_light_factor - 0.24).abs() < 1e-6);
        assert_eq!(midnight.sky_light_color, NIGHT_SKY_LIGHT_COLOR);
        assert!((midnight.sun_angle - 180.0).abs() < 1e-3, "{}", midnight.sun_angle);
        assert!((midnight.moon_angle - 360.0).abs() < 1e-3, "{}", midnight.moon_angle);
        assert!((midnight.star_brightness - 0.5).abs() < 1e-6);
        assert!(!midnight.is_day());
        // the view's night (the clock's moon, the scrubber's dark spans) is the day's dark half
        assert!(at(DUSK - 1, 0.0, 0.0).is_day() && !at(DUSK + 1, 0.0, 0.0).is_day());
        assert!(!at(24000 + DAWN - 1, 0.0, 0.0).is_day() && at(24000 + DAWN + 1, 0.0, 0.0).is_day());
        // the lightmap at noon is the one the view baked before the day cycle (`light::lightmap`)
        for (s, b) in [(15.0, 0.0), (14.75, 0.0), (8.0, 3.0), (0.0, 15.0), (12.5, 7.25)] {
            let (x, y) = (lightmap(&noon, s, b), crate::light::lightmap(s, b));
            assert!((0..3).all(|k| (x[k] - y[k]).abs() < 1e-5), "({s}, {b}): {x:?} vs {y:?}");
        }
        // at midnight the sky's light falls, a torch's stays
        assert!(lightmap(&midnight, 15.0, 0.0)[0] < 0.5 * lightmap(&noon, 15.0, 0.0)[0]);
        assert_eq!(lightmap(&midnight, 0.0, 14.0), lightmap(&noon, 0.0, 14.0));
        // rain darkens the day
        assert!(at(6000, 1.0, 0.0).sky_light_factor < 0.8);
    }

    /// The tracks against the game's own file (`timeline/day.json` of the server jar, decompiled
    /// once by `mctas src`; skipped where it is not).
    #[test]
    fn the_tracks_are_the_games() {
        let Ok(home) = std::env::var("HOME") else { return };
        let Ok(text) = std::fs::read_to_string(format!("{home}/.cache/mctas/versions/26.3/src/data/minecraft/timeline/day.json")) else { return };
        let flat: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let keys = |name: &str| -> String {
            let at = flat.find(&format!("\"minecraft:visual/{name}\":")).unwrap_or_else(|| panic!("no track {name}"));
            let rest = &flat[at..];
            let k = rest.find("\"keyframes\":[").unwrap() + 13;
            rest[k..k + rest[k..].find(']').unwrap()].to_string()
        };
        let as_text = |v: Vec<String>| v.join(",");
        let hex3 = |c: [f32; 3]| format!("\"#{:02x}{:02x}{:02x}\"", (c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8);
        let float = |v: f32| if v == v.trunc() { format!("{v:.1}") } else { format!("{v}") };
        assert_eq!(keys("sky_color"), as_text(SKY_COLOR_TRACK.iter().map(|(t, c)| format!("{{\"ticks\":{t},\"value\":{}}}", hex3(*c))).collect()));
        assert_eq!(keys("fog_color"), as_text(FOG_COLOR_TRACK.iter().map(|(t, c)| format!("{{\"ticks\":{t},\"value\":{}}}", hex3(*c))).collect()));
        assert_eq!(keys("sky_light_color"), as_text(SKY_LIGHT_COLOR_TRACK.iter().map(|(t, c)| format!("{{\"ticks\":{t},\"value\":{}}}", hex3(*c))).collect()));
        assert_eq!(keys("sky_light_factor"), as_text(SKY_LIGHT_FACTOR_TRACK.iter().map(|(t, v)| format!("{{\"ticks\":{t},\"value\":{}}}", float(*v))).collect()));
        assert_eq!(keys("star_brightness"), as_text(STAR_BRIGHTNESS_TRACK.iter().map(|(t, v)| format!("{{\"ticks\":{t},\"value\":{}}}", float(*v))).collect()));
        assert_eq!(keys("sunrise_sunset_color"), as_text(SUNRISE_SUNSET_TRACK.iter().map(|(t, c)| format!("{{\"ticks\":{t},\"value\":\"#{c:08x}\"}}")).collect()));
        assert_eq!(keys("sun_angle"), "{\"ticks\":6000,\"value\":360.0},{\"ticks\":6000,\"value\":0.0}");
        assert_eq!(keys("moon_angle"), "{\"ticks\":6000,\"value\":540.0},{\"ticks\":6000,\"value\":180.0}");
    }
}

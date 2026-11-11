//! Declared flights (ironspec's ruling, Nov 11 01:02): a design states the flights it intends, a
//! cart leaving its take-off rail into the air and landing on a declared rail, each with a
//! maximum airtime. The off-the-rails check exempts a declared flight only when the cart lands on
//! one of the flight's landing cells within the bound, moving forward, its rider seated and not on
//! top of another cart. Every other off-rail stretch (3 ticks with no rail in the cart's cell or
//! the one below) is a derail, as before. The design file carries its flights as a header line
//! `flights: X,Y,Z>X,Y,Z|X,Y,Z@AIR; ...` (take-off > landing cells @ maximum airtime in gt).

use redsim::entity::EntityKind;
use redsim::pos::BlockPos;
use redsim::world::World;

/// One intended flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    /// The rail cell the cart leaves from.
    pub take_off: [i32; 3],
    /// The rail cells it may land on.
    pub landing: Vec<[i32; 3]>,
    /// The longest airtime, in gt, before an off-rail cart is a derail.
    pub max_air: u32,
}

impl Flight {
    /// Moved by (dx, dz).
    pub fn shifted(&self, dx: i32, dz: i32) -> Flight {
        let m = |c: [i32; 3]| [c[0] + dx, c[1], c[2] + dz];
        Flight { take_off: m(self.take_off), landing: self.landing.iter().map(|c| m(*c)).collect(), max_air: self.max_air }
    }
}

/// The header line's value for a list of flights.
pub fn format(fl: &[Flight]) -> String {
    let c = |c: &[i32; 3]| format!("{},{},{}", c[0], c[1], c[2]);
    fl.iter().map(|f| format!("{}>{}@{}", c(&f.take_off), f.landing.iter().map(c).collect::<Vec<_>>().join("|"), f.max_air)).collect::<Vec<_>>().join("; ")
}

/// Parses a `flights:` header value.
pub fn parse(s: &str) -> Result<Vec<Flight>, String> {
    let cell = |t: &str| -> Result<[i32; 3], String> {
        let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>().map_err(|e| format!("flights: {t}: {e}"))).collect::<Result<_, _>>()?;
        match v.as_slice() {
            [x, y, z] => Ok([*x, *y, *z]),
            _ => Err(format!("flights: {t}: give X,Y,Z")),
        }
    };
    let mut out = Vec::new();
    for item in s.split(';').map(str::trim).filter(|t| !t.is_empty()) {
        let (cells, air) = item.split_once('@').ok_or_else(|| format!("flights: {item}: no @AIR"))?;
        let (from, to) = cells.split_once('>').ok_or_else(|| format!("flights: {item}: no >"))?;
        out.push(Flight { take_off: cell(from)?, landing: to.split('|').map(cell).collect::<Result<_, _>>()?, max_air: air.trim().parse().map_err(|e| format!("flights: {item}: {e}"))? });
    }
    Ok(out)
}

#[derive(Clone, Debug, Default)]
struct CartState {
    last_rail: Option<[i32; 3]>,
    off: u32,
    in_flight: Option<usize>,
    reported: bool,
}

/// The off-the-rails watch with declared flights: feed it every cart every tick.
#[derive(Clone, Debug, Default)]
pub struct RailWatch {
    flights: Vec<Flight>,
    st: Vec<CartState>,
    /// Each derail: when, which cart, why.
    pub derails: Vec<String>,
    /// Declared flights that landed as declared.
    pub landed: usize,
}

impl RailWatch {
    pub fn new(flights: Vec<Flight>) -> RailWatch {
        RailWatch { flights, ..Default::default() }
    }

    /// One cart's tick: `k` its index (the state slot and the name in the report), `id` the cart,
    /// `rider` its rider (seated at the landing), `when` the tick's name.
    pub fn observe(&mut self, w: &World, k: usize, id: u32, rider: Option<u32>, when: &str) {
        let Some(e) = w.entities.list.iter().find(|e| e.id == id && !e.removed) else { return };
        if self.st.len() <= k {
            self.st.resize(k + 1, CartState::default());
        }
        let cell = [e.pos.x.floor() as i32, e.pos.y.floor() as i32, e.pos.z.floor() as i32];
        let below = [cell[0], cell[1] - 1, cell[2]];
        let rail_at = |c: [i32; 3]| redsim::rails::is_rail(&w.get(BlockPos::new(c[0], c[1], c[2])));
        let rc = if rail_at(cell) { Some(cell) } else if rail_at(below) { Some(below) } else { None };
        let s = &mut self.st[k];
        match rc {
            Some(rc) => {
                if let Some(fi) = s.in_flight.take() {
                    let f = &self.flights[fi];
                    let mut why: Vec<String> = Vec::new();
                    if !f.landing.contains(&rc) {
                        why.push(format!("landed on {},{},{}, not on its declared landing", rc[0], rc[1], rc[2]));
                    }
                    let to = f.landing.first().copied().unwrap_or(rc);
                    let (dx, dz) = ((to[0] - f.take_off[0]).signum() as f64, (to[2] - f.take_off[2]).signum() as f64);
                    if e.delta_movement.x * dx + e.delta_movement.z * dz <= 0.0 {
                        why.push("not moving forward".into());
                    }
                    if rider.is_some_and(|r| !e.passengers.contains(&r)) {
                        why.push("rider not seated".into());
                    }
                    let on_top = w.entities.list.iter().any(|o| o.id != id && !o.removed && matches!(o.kind, EntityKind::Minecart(_)) && (o.pos.x - e.pos.x).abs() < 0.98 && (o.pos.z - e.pos.z).abs() < 0.98 && e.pos.y - o.pos.y > 0.3);
                    if on_top {
                        why.push("on top of another cart".into());
                    }
                    if why.is_empty() {
                        self.landed += 1;
                    } else {
                        self.derails.push(format!("{when}: cart {k} {} ({:.2},{:.2},{:.2})", why.join(", "), e.pos.x, e.pos.y, e.pos.z));
                    }
                }
                s.off = 0;
                s.last_rail = Some(rc);
                s.reported = false;
            }
            None => {
                s.off += 1;
                if s.off == 1 {
                    s.in_flight = self.flights.iter().position(|f| Some(f.take_off) == s.last_rail);
                }
                match s.in_flight {
                    Some(fi) => {
                        if s.off > self.flights[fi].max_air && !s.reported {
                            self.derails.push(format!("{when}: cart {k} off the rails past its declared flight's {} gt ({:.2},{:.2},{:.2})", self.flights[fi].max_air, e.pos.x, e.pos.y, e.pos.z));
                            s.reported = true;
                            s.in_flight = None;
                        }
                    }
                    None => {
                        if s.off == 3 && !s.reported {
                            self.derails.push(format!("{when}: cart {k} off the rails ({:.2},{:.2},{:.2})", e.pos.x, e.pos.y, e.pos.z));
                            s.reported = true;
                        }
                    }
                }
            }
        }
    }

    /// The hard line.
    pub fn line(&self) -> String {
        format!("carts off the rails: {}{} ({} declared flights landed as declared)", self.derails.len(), if self.derails.is_empty() { String::new() } else { format!(" ({})", self.derails.iter().take(6).cloned().collect::<Vec<_>>().join("; ")) }, self.landed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flights_round_trip() {
        let f = vec![Flight { take_off: [0, 10, 21], landing: vec![[0, 9, 23], [0, 9, 24]], max_air: 6 }, Flight { take_off: [-3, 2, 0], landing: vec![[-2, 1, 0]], max_air: 4 }];
        let s = format(&f);
        assert_eq!(s, "0,10,21>0,9,23|0,9,24@6; -3,2,0>-2,1,0@4");
        assert_eq!(parse(&s).unwrap(), f);
        assert_eq!(f[0].shifted(6, 6).take_off, [6, 10, 27]);
    }
}

//! The 3D view's bedrock-breaker press (`Session::press_swap`, the viewer's tap on a swap piston):
//! pressed in an armed tick, the bedrock under it disappears; pressed in other ticks it stays, and
//! a tap on the piston left facing down puts the piston as built back.

use mcview::session::{Options, Session, bp};

const DESIGN: &str = include_str!("../../../designs/circuits/bedrock/tracks/bedrock/bedrock-breaker-11b.txt");

fn run(press_at: u32) -> (bool, Session) {
    let mut s = Session::new(DESIGN, &Options::live()).expect("the design loads");
    let swap = s.swaps.first().expect("a swap piston").0;
    let target = s.design.port("bedrock").expect("port bedrock");
    for _ in 0..press_at {
        s.step();
    }
    let pressed = s.press_swap(swap);
    for _ in 0..3 {
        s.step();
    }
    let gone = !s.world.get(bp(target)).to_string().starts_with("minecraft:bedrock");
    (pressed && gone, s)
}

#[test]
fn a_press_in_an_armed_tick_removes_the_bedrock() {
    let armed: Vec<u32> = (80..92).filter(|&t| run(t).0).collect();
    let early: Vec<u32> = (1..40).filter(|&t| run(t).0).collect();
    eprintln!("armed ticks in 80..92 (presses after that many steps): {armed:?}; in 1..40: {early:?}");
    assert!(!armed.is_empty(), "no tick of 80..92 is armed");
    assert!(armed.len() <= 3, "too many armed ticks: {armed:?}");
    assert!(armed.windows(2).all(|w| w[1] - w[0] >= 5), "armed ticks too close: {armed:?}");
    // the badge says ARMED exactly in the ticks a press works, and says so after the press
    for t in 1..40 {
        let mut s = Session::new(DESIGN, &Options::live()).expect("the design loads");
        for _ in 0..t {
            s.step();
        }
        let words = s.swap_status().unwrap_or_default();
        assert_eq!(words.starts_with("ARMED"), early.contains(&t), "tick {t}: badge {words:?}, armed ticks {early:?}");
    }
    let (_, s) = run(early[0]);
    assert_eq!(s.swap_status().as_deref(), Some("bedrock removed"));
}

#[test]
fn a_miss_is_fixed_by_a_second_tap() {
    let missed = (80..92).find(|&t| !run(t).0).expect("a tick that is not armed");
    let (_, mut s) = run(missed);
    let swap = s.swaps[0].0;
    let built = s.swaps[0].1.clone();
    assert!(s.world.get(bp(swap)).to_string().contains("facing=down"), "the miss left a piston facing down");
    assert!(s.press_swap(swap), "the fix taps the piston");
    let now = s.world.get(bp(swap)).to_string();
    assert_eq!(now.contains("facing=west"), built.contains("facing=west"), "the fix puts the piston as built back: {now}");
}

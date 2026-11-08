use redsim::{BlockPos, World};
use redsim::entity::{self, leash};

fn world() -> World { World::new(BlockPos::new(-16, -140, -16), [32, 160, 32]) }

fn boat(w: &mut World, x: f64, y: f64) -> u32 {
    entity::spawn(w, "minecraft:oak_boat", [x, y, 0.0], None).unwrap()
}

#[test]
fn destroying_leashee_refuses_unmodeled_drop_atomically() {
    let mut w = world();
    let a = boat(&mut w, 0.0, 5.0);
    let b = boat(&mut w, 7.0, 5.0);
    leash::attach(&mut w, a, b).unwrap();
    let error = entity::kill(&mut w, a).unwrap_err();
    assert!(error.contains("unseeded lead ItemEntity"));
    assert!(!w.entities.list[0].removed);
    assert!(w.entities.list[0].leash.is_some());
}

#[test]
fn destroying_holder_is_detected_when_leashee_ticks() {
    let mut w = world();
    let a = boat(&mut w, 0.0, 5.0);
    let b = boat(&mut w, 7.0, 5.0);
    leash::attach(&mut w, a, b).unwrap();
    entity::kill(&mut w, b).unwrap();
    leash::tick(&mut w, 0);
    assert!(w.entities.list[0].leash.is_none());
    assert!(w.errors.iter().any(|e| e.contains("dropped lead")));
}

#[test]
fn invalid_remove_matches_attachment_type_and_liveness_checks() {
    let mut w = world();
    let live = boat(&mut w, 0.0, 5.0);
    leash::remove(&mut w, live).unwrap(); // live unlinked boat is a valid no-op
    for kind in ["minecraft:armor_stand", "minecraft:minecart", "minecraft:item"] {
        let nbt = (kind == "minecraft:item").then_some("{Item:{id:\"minecraft:stone\",count:1}}");
        let id = entity::spawn(&mut w, kind, [2.0, 5.0, 0.0], nbt).unwrap();
        assert!(leash::remove(&mut w, id).is_err(), "{kind}");
        assert!(leash::attach(&mut w, id, live).is_err(), "{kind}");
    }
    let gone = boat(&mut w, 4.0, 5.0);
    entity::kill(&mut w, gone).unwrap();
    assert!(leash::remove(&mut w, gone).is_err());
    assert!(leash::remove(&mut w, 999).is_err());
}

#[test]
fn below_world_removal_also_fails_closed_for_a_lead_drop() {
    let mut w = world();
    let a = boat(&mut w, 0.0, -129.0);
    let b = boat(&mut w, 0.0, -120.0);
    leash::attach(&mut w, a, b).unwrap();
    entity::base_tick(&mut w, 0);
    assert!(w.entities.list[0].removed);
    assert!(w.entities.list[0].leash.is_none());
    assert!(w.errors.iter().any(|e| e.contains("dropped lead")));
}

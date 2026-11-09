//! A drop that burns up in lava leaves evidence: `MobEvent::Burnt` (the item's id, its item and
//! count), pushed when `ItemEntity.hurtServer` takes its health to 0. The mob-farm judge counts a
//! farm drop's burnt iron from it (brief farm-iron rev 9 iteration 11: a measured loss, not a lost
//! drop): the drop is gone within its tick, before the judge's per-tick drop check can see it.
use redsim::entity::item::ItemState;
use redsim::entity::mob::MobEvent;
use redsim::entity::{EntityKind, Vec3};
use redsim::{Block, BlockPos, World};

fn world(lava: bool) -> World {
    let mut w = World::new(BlockPos::new(-8, 0, -8), [16, 16, 16]);
    w.mob.seed(13, 0);
    for x in -2..=2 {
        for z in -2..=2 {
            w.set_block(BlockPos::new(x, 0, z), Block::parse("minecraft:stone").unwrap(), 2);
        }
    }
    if lava {
        w.set_block(BlockPos::new(0, 1, 0), Block::Lava { level: 0 }, 2);
    }
    w
}

fn add_ingots(w: &mut World, count: u8) -> u32 {
    let state = ItemState { item: "minecraft:iron_ingot".to_string(), count, pickup_delay: 10, age: 0, payload: 0, health: 5, ..Default::default() };
    w.entities.add(EntityKind::Item(state), Vec3::new(0.5, 1.6, 0.5), 0.0)
}

fn burnt(w: &World) -> Vec<(u32, String, u32)> {
    w.mob.events.iter().filter_map(|e| if let MobEvent::Burnt { item, what, count, .. } = e { Some((*item, what.clone(), *count)) } else { None }).collect()
}

#[test]
fn a_drop_that_burns_up_in_lava_records_its_burn() {
    let mut w = world(true);
    let id = add_ingots(&mut w, 4);
    for _ in 0..40 {
        w.tick();
    }
    assert!(!w.entities.list.iter().any(|e| e.id == id), "the drop should have burnt up in the lava");
    assert_eq!(burnt(&w), vec![(id, "minecraft:iron_ingot".to_string(), 4)]);
}

#[test]
fn a_drop_beside_no_lava_records_no_burn() {
    // the positive control: the same drop on stone lives and records nothing
    let mut w = world(false);
    let id = add_ingots(&mut w, 4);
    for _ in 0..40 {
        w.tick();
    }
    assert!(w.entities.list.iter().any(|e| e.id == id), "the drop on stone should still be there");
    assert!(burnt(&w).is_empty());
}

//! A mob's loot pickup (26.3 `Mob.aiStep`'s looting, after `burnUndead`): a mob that
//! `canPickUpLoot`, alive and not dying, with mob griefing on, tries every item entity its box
//! grown by (1, 0, 1) meets (`getEntitiesOfClass`: the level's section order), each not removed,
//! not empty, past its pickup delay, that it `wantsToPickUp`: `pickUpItem` ->
//! `equipItemIfPossible` (the main hand takes the whole stack, `setGuaranteedDrop`, the mob made
//! persistent, the item entity discarded). Only the main hand is modeled; an item for an armor
//! slot (armor, an elytra, a head, a carved pumpkin) or the off hand (a shield), and two weapons
//! of the same attack damage (their enchantments and wear decide), run on the legacy model (no
//! pickup), named in the run's legacy list. Body and saddle equipment (a llama's carpet, a
//! harness, a saddle, horse, wolf and nautilus armor) is never picked up (`canBeEquippedBy`).

use crate::entity::mob::{Held, MobEvent, mob, mob_mut};
use crate::entity::{EntityKind, Vec3};
use crate::world::World;

const COLORS: [&str; 16] = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"];

/// Where `getEquipmentSlotForItem` and `isEquippableInSlot` put an item for a zombie or skeleton.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Slot {
    MainHand,
    Armor,
    OffHand,
    Never,
}

fn slot_of(id: &str) -> Slot {
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    if id.ends_with("_helmet") || id.ends_with("_chestplate") || id.ends_with("_leggings") || id.ends_with("_boots") {
        return Slot::Armor;
    }
    if matches!(id, "elytra" | "carved_pumpkin" | "skeleton_skull" | "wither_skeleton_skull" | "player_head" | "zombie_head" | "creeper_head" | "dragon_head" | "piglin_head") {
        return Slot::Armor;
    }
    if id == "shield" {
        return Slot::OffHand;
    }
    let colored = |suffix: &str| COLORS.iter().any(|c| id.strip_prefix(c).is_some_and(|r| r == suffix));
    if colored("_carpet") || colored("_harness") || id == "saddle" || id.ends_with("_horse_armor") || id == "wolf_armor" || id.ends_with("_nautilus_armor") {
        return Slot::Never;
    }
    Slot::MainHand
}

/// A tool's or weapon's main-hand ATTACK_DAMAGE modifier (26.3 `ToolMaterial`: the baseline plus
/// the material's bonus, in floats; `TridentItem` 8, `MaceItem` 5), and whether the item carries
/// one at all (a golden or wooden spear's is 0).
fn attack_bonus(id: &str) -> (f64, bool) {
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    match id {
        "trident" => return (8.0, true),
        "mace" => return (5.0, true),
        _ => {}
    }
    let Some((material, tool)) = id.split_once('_') else { return (0.0, false) };
    let bonus: f32 = match material {
        "wooden" | "golden" => 0.0,
        "stone" | "copper" => 1.0,
        "iron" => 2.0,
        "diamond" => 3.0,
        "netherite" => 4.0,
        _ => return (0.0, false),
    };
    let base: f32 = match (tool, material) {
        ("sword", _) => 3.0,
        ("shovel", _) => 1.5,
        ("pickaxe", _) => 1.0,
        ("spear", _) => 0.0,
        ("axe", "wooden" | "iron" | "golden") => 6.0,
        ("axe", "stone" | "copper") => 7.0,
        ("axe", _) => 5.0,
        ("hoe", "wooden" | "golden") => 0.0,
        ("hoe", "stone" | "copper") => -1.0,
        ("hoe", "iron") => -2.0,
        ("hoe", "diamond") => -3.0,
        ("hoe", _) => -4.0,
        _ => return (0.0, false),
    };
    ((base + bonus) as f64, true)
}

/// A main-hand item's attack damage bonus (`Mob.doHurtTarget`'s ATTACK_DAMAGE with it).
pub fn weapon_damage(id: &str) -> f64 {
    attack_bonus(id).0
}

fn is_skeleton(id: &str) -> bool {
    matches!(id, "skeleton" | "stray" | "bogged")
}

fn is_drowned(id: &str) -> bool {
    matches!(id, "drowned" | "baby_drowned")
}

fn is_piglin(id: &str) -> bool {
    matches!(id, "zombified_piglin" | "baby_zombified_piglin")
}

/// `getPreferredWeaponType`: a skeleton's `#skeleton_preferred_weapons` (a bow), a drowned's
/// `#drowned_preferred_weapons` (a trident).
fn preferred(ty: &str) -> Option<&'static str> {
    if is_skeleton(ty) {
        Some("minecraft:bow")
    } else if is_drowned(ty) {
        Some("minecraft:trident")
    } else {
        None
    }
}

fn is_spear(id: &str) -> bool {
    id.strip_prefix("minecraft:").unwrap_or(id).ends_with("_spear")
}

/// `canHoldItem` (Zombie: no egg for a baby riding something) and `wantsToPickUp` (Zombie: no
/// glow ink sac; a drowned or a skeleton: no spear; a zombified piglin: `canHoldItem` alone).
fn wants(w: &World, i: usize, item: &str) -> bool {
    let ty = mob(w, i).ty.def().id;
    let zombie = crate::entity::mob::natural::is_zombie_class(mob(w, i).ty);
    let egg = matches!(item, "minecraft:egg" | "minecraft:blue_egg" | "minecraft:brown_egg");
    let can_hold = !(zombie && egg && ty.starts_with("baby_") && w.entities.list[i].vehicle.is_some());
    if is_piglin(ty) {
        return can_hold;
    }
    if (is_drowned(ty) || is_skeleton(ty)) && is_spear(item) {
        return false;
    }
    if zombie && item == "minecraft:glow_ink_sac" {
        return false;
    }
    can_hold
}

fn legacy(w: &mut World, what: &'static str) {
    w.mob.ai.legacy.insert(what);
}

/// The looting step of mob `i`'s `aiStep` (the caller checks the mob AI is on).
pub fn pick_up(w: &mut World, i: usize) {
    let m = mob(w, i);
    if !m.can_pick_up_loot || m.dead || m.health <= 0.0 || w.entities.list[i].removed || !w.mob.ai.mob_griefing {
        return;
    }
    let ty = m.ty.def().id;
    if !crate::entity::mob::natural::is_zombie_class(m.ty) && !is_skeleton(ty) {
        legacy(w, "a mob's loot pickup (not a zombie or skeleton)");
        return;
    }
    let reach = w.entities.list[i].bb.inflate(1.0, 0.0, 1.0);
    let items = w.get_entities(reach, |e| matches!(&e.kind, EntityKind::Item(s) if s.count > 0 && s.pickup_delay == 0));
    for j in items {
        if w.entities.list[j].removed {
            continue;
        }
        let EntityKind::Item(s) = &w.entities.list[j].kind else { continue };
        let (item, count) = (s.item.clone(), s.count);
        if count == 0 || s.pickup_delay > 0 || !wants(w, i, &item) {
            continue;
        }
        pick_up_item(w, i, j, &item, count);
    }
}

/// `Mob.pickUpItem` -> `equipItemIfPossible` for the main hand.
fn pick_up_item(w: &mut World, i: usize, j: usize, item: &str, count: u8) {
    match slot_of(item) {
        Slot::Never => return,
        Slot::Armor => return legacy(w, "a mob picking up armor (its armor slots)"),
        Slot::OffHand => return legacy(w, "a mob picking up a shield (its off hand)"),
        Slot::MainHand => {}
    }
    let current = mob(w, i).mainhand.clone();
    let replace = match &current {
        None => true,
        Some(cur) => {
            // compareWeapons: the preferred weapon type first, then the attack damage, then
            // canReplaceEqualItem (enchantments, wear, a custom name)
            let ty = mob(w, i).ty.def().id;
            let pref = preferred(ty);
            if pref.is_some_and(|p| cur.item == p && item != p) {
                false
            } else if pref.is_some_and(|p| cur.item != p && item == p) {
                true
            } else {
                let (new_bonus, new_weapon) = attack_bonus(item);
                let (old_bonus, _) = attack_bonus(&cur.item);
                if new_bonus != old_bonus {
                    new_bonus > old_bonus
                } else if new_weapon {
                    legacy(w, "a mob comparing two weapons of the same attack damage (their enchantments and wear)");
                    false
                } else {
                    // a plain item against the hand's: no enchantments, no wear, no name on the
                    // ground's; a spawn weapon's enchantments only keep it
                    false
                }
            }
        }
    };
    if !replace {
        return;
    }
    let pos = w.entities.list[i].pos;
    let by = w.entities.list[i].id;
    if let Some(cur) = current {
        // the hand's item drops when max(nextFloat() - 0.1, 0) < its drop chance (2 for a
        // guaranteed one, else the default 0.085)
        let chance = if cur.guaranteed { 2.0f32 } else { 0.085f32 };
        let roll = w.entities.list[i].random.as_mut().map_or(1.0, |r| r.next_float());
        if (roll - 0.1f32).max(0.0) < chance {
            crate::entity::mob::living::spawn_item_at(w, pos, by, intern(&cur.item), cur.count);
        }
    }
    {
        let m = mob_mut(w, i);
        m.mainhand = Some(Held { item: item.to_string(), count, guaranteed: true, enchanted: false });
        m.persistence_required = true;
    }
    // take + shrink: the main hand took the whole stack, the item entity is discarded
    let item_id = w.entities.list[j].id;
    w.entities.list[j].removed = true;
    let t = w.time;
    w.mob.events.push(MobEvent::PickedUp { t, by, item: item_id, what: item.to_string(), count: count as u32 });
}

/// `Mob.dropCustomDeathLoot` for the main hand (after the loot table's drops, only when the mob
/// drops loot): a guaranteed item (one it picked up) draws `nextFloat() < 2` and drops at its
/// position; a spawn weapon (0.085) drops only on a player's kill (none here: no draw).
pub fn drop_hand(w: &mut World, i: usize) {
    let Some(held) = mob(w, i).mainhand.clone() else { return };
    if !held.guaranteed {
        return;
    }
    if let Some(r) = w.entities.list[i].random.as_mut() {
        r.next_float();
    }
    let pos: Vec3 = w.entities.list[i].pos;
    let by = w.entities.list[i].id;
    crate::entity::mob::living::spawn_item_at(w, pos, by, intern(&held.item), held.count);
    mob_mut(w, i).mainhand = None;
}

/// An item id as a `&'static str` (the drop events keep their item names so).
fn intern(s: &str) -> &'static str {
    use std::sync::{Mutex, OnceLock};
    static NAMES: OnceLock<Mutex<std::collections::HashSet<&'static str>>> = OnceLock::new();
    let mut set = NAMES.get_or_init(Default::default).lock().expect("names");
    if let Some(n) = set.get(s) {
        return n;
    }
    let n: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(n);
    n
}

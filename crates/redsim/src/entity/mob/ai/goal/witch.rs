//! The witch (`Witch`: a raider; natural ones join no raid): `Witch.aiStep`'s potion choice before
//! `super.aiStep()` (four `nextFloat` rolls in order, each before its condition: water breathing
//! with the eye in water, fire resistance when burning, healing when hurt, swiftness at a far
//! target; then the particle event's roll; a chosen potion, its drinking, is refused), its goals
//! (`FloatGoal`, `RangedAttackGoal` refused once it has a target, `WaterAvoidingRandomStrollGoal`,
//! `LookAtPlayerGoal` 8, `RandomLookAroundGoal`; the raid goals never run outside a raid and draw
//! nothing) and targets (`HurtByTargetGoal`, `NearestHealableRaiderTargetGoal`: its
//! `nextBoolean` before the raid check, `NearestAttackableWitchTargetGoal` of players, 10).

use super::{Goal, LOOK, MOVE, TARGET};
use crate::entity::mob::ai::ai;
use crate::entity::mob::mob;
use crate::world::World;

#[derive(Clone, PartialEq, Debug)]
pub enum WitchGoal {
    RangedAttack,
    HealRaider,
}

impl WitchGoal {
    pub fn flags(&self) -> u8 {
        match self {
            WitchGoal::RangedAttack => MOVE | LOOK,
            WitchGoal::HealRaider => TARGET,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            WitchGoal::RangedAttack => "RangedAttackGoal",
            WitchGoal::HealRaider => "NearestHealableRaiderTargetGoal",
        }
    }
}

/// `Witch.registerGoals` (with `Raider`'s and `PatrollingMonster`'s, which draw nothing and never
/// run outside a raid or a patrol: left out).
pub fn register(a: &mut crate::entity::mob::ai::MobAi) {
    use super::look::{LookAtPlayer, LookType, RandomLookAround};
    use super::stroll::RandomStroll;
    use super::targets::{HurtByTarget, NearestAttackableTarget, TargetType};
    a.goals.add(1, Goal::Float);
    a.goals.add(2, Goal::Witch(WitchGoal::RangedAttack));
    a.goals.add(2, Goal::RandomStroll(RandomStroll::water_avoiding(1.0)));
    a.goals.add(3, Goal::LookAtPlayer(LookAtPlayer::new(LookType::Player, 8.0)));
    a.goals.add(3, Goal::RandomLookAround(RandomLookAround::default()));
    a.targets.add(1, Goal::HurtByTarget(HurtByTarget::new(false)));
    a.targets.add(2, Goal::Witch(WitchGoal::HealRaider));
    a.targets.add(3, Goal::NearestAttackableTarget(NearestAttackableTarget::new(TargetType::Player, 10, true, false)));
}

fn refuse(w: &mut World, why: &str) {
    if !w.errors.iter().any(|e| e == why) {
        w.errors.push(why.to_string());
    }
}

pub fn can_use(w: &mut World, i: usize, g: &mut WitchGoal) -> bool {
    match g {
        WitchGoal::RangedAttack => {
            if crate::entity::mob::ai::target::get_target(w, i).is_some() {
                refuse(w, "a witch's ranged attack (its thrown potions) is not modeled yet");
            }
            false
        }
        // its cooldown never starts outside a raid: nextBoolean, then no active raid
        WitchGoal::HealRaider => {
            if let Some(r) = w.entities.list[i].random.as_mut() {
                r.next_bool();
            }
            false
        }
    }
}

/// `Witch.aiStep` before `super.aiStep()`, a living witch not drinking: the potion's four rolls in
/// the game's order (each drawn before its condition), then the particle event's.
pub fn before_ai_step(w: &mut World, i: usize) {
    if mob(w, i).ty != crate::entity::mob::types::MobType::Witch || mob(w, i).health <= 0.0 {
        return;
    }
    if let Some((potion, t)) = crate::entity::mob::ai::ai(w, i).witch_drink {
        // drinking: usingTime-- <= 0 finishes it (the potion's effects, the speed back)
        if t <= 0 {
            crate::entity::mob::ai::ai_mut(w, i).witch_drink = None;
            let e = &mut crate::entity::mob::mob_mut(w, i).effects;
            match potion {
                0 => e.water_breathing = 3600,
                1 => e.fire_resistance = 3600,
                2 => e.instant_health = true,
                _ => e.speed = 3600,
            }
        } else {
            crate::entity::mob::ai::ai_mut(w, i).witch_drink = Some((potion, t - 1));
        }
    } else {
        let eye_in_water = w.entities.list[i].fluid.eye_in_water();
        // isOnFire, or getLastDamageSource() a fire one (within 40 ticks)
        let now = w.time;
        let burning = mob(w, i).fire_ticks > 0 || mob(w, i).last_damage.is_some_and(|(t, s)| now - t <= 40 && s.is_fire());
        let hurt = mob(w, i).health < mob(w, i).ty.def().max_health;
        let fx = mob(w, i).effects;
        let pos = w.entities.list[i].pos;
        let far_target = crate::entity::mob::ai::target::get_target(w, i)
            .and_then(|id| crate::entity::mob::ai::target::who(w, id))
            .is_some_and(|t| crate::entity::mob::ai::target::pos_of(w, t).distance_to_sqr(pos) > 121.0);
        let Some(r) = w.entities.list[i].random.as_mut() else { return };
        let potion = if r.next_float() < 0.15 && eye_in_water && fx.water_breathing == 0 {
            Some(0u8)
        } else if r.next_float() < 0.15 && burning && fx.fire_resistance == 0 {
            Some(1)
        } else if r.next_float() < 0.05 && hurt {
            Some(2)
        } else if r.next_float() < 0.5 && far_target && fx.speed == 0 {
            Some(3)
        } else {
            None
        };
        if let Some(p) = potion {
            // the potion's use duration (32 ticks), the drink sound's pitch, the -0.25 speed
            r.next_float();
            crate::entity::mob::ai::ai_mut(w, i).witch_drink = Some((p, 32));
        }
    }
    if let Some(r) = w.entities.list[i].random.as_mut() {
        r.next_float();
    }
    let _ = ai;
    let _ = refuse;
}

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
    let eye_in_water = w.entities.list[i].fluid.eye_in_water();
    let burning = mob(w, i).fire_ticks > 0;
    let hurt = mob(w, i).health < mob(w, i).ty.def().max_health;
    let far_target = crate::entity::mob::ai::target::get_target(w, i).is_some();
    let Some(r) = w.entities.list[i].random.as_mut() else { return };
    let potion = if r.next_float() < 0.15 && eye_in_water {
        true
    } else if r.next_float() < 0.15 && burning {
        true
    } else if r.next_float() < 0.05 && hurt {
        true
    } else {
        r.next_float() < 0.5 && far_target
    };
    r.next_float();
    if potion {
        refuse(w, "a witch drinking a potion is not modeled yet");
    }
    let _ = ai;
}

use std::sync::Arc;
use std::sync::atomic::Ordering;

use pumpkin_data::entity::EntityType;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockStateId};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;
use rand::{Rng, RngExt};
use uuid::Uuid;

use crate::block::{
    BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnEntityCollisionArgs,
    OnScheduledTickArgs, PlacedArgs,
};
use crate::entity::r#type::from_type;
use crate::world::World;

const MIN_TADPOLES_SPAWN: u32 = 2;
const MAX_TADPOLES_SPAWN: u32 = 5;
const DEFAULT_MIN_HATCH_TICK_DELAY: u32 = 3600;
const DEFAULT_MAX_HATCH_TICK_DELAY: u32 = 12000;
const TADPOLE_HITBOX_CENTER: f32 = 0.2;

#[pumpkin_block("minecraft:frogspawn")]
pub struct FrogspawnBlock;

impl FrogspawnBlock {
    fn can_survive(world: &World, pos: &BlockPos) -> bool {
        Self::may_place_on(world, &pos.down())
    }

    fn may_place_on(world: &World, pos: &BlockPos) -> bool {
        // `FluidState#is` checks the exact fluid, so flowing water does not count.
        let (_, fluid_state) = world.get_fluid_and_fluid_state(pos);
        let (_, fluid_above) = world.get_fluid_and_fluid_state(&pos.up());
        (Fluid::from_state_id(fluid_state.block_state_id)
            .is_some_and(|fluid| fluid.has_tag(&tag::Fluid::MINECRAFT_SUPPORTS_FROGSPAWN))
            || world
                .get_block(pos)
                .has_tag(&tag::Block::MINECRAFT_SUPPORTS_FROGSPAWN))
            && fluid_above.is_empty
    }

    fn get_frogspawn_hatch_delay() -> u32 {
        rand::rng().random_range(DEFAULT_MIN_HATCH_TICK_DELAY..DEFAULT_MAX_HATCH_TICK_DELAY)
    }

    fn hatch_frogspawn(world: &Arc<World>, pos: &BlockPos) {
        Self::destroy_block(world, pos);
        world.play_block_sound(Sound::BlockFrogspawnHatch, SoundCategory::Blocks, *pos);
        Self::spawn_tadpoles(world, pos);
    }

    fn destroy_block(world: &Arc<World>, pos: &BlockPos) {
        world.break_block(pos, None, BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_DROPS);
    }

    fn spawn_tadpoles(world: &Arc<World>, pos: &BlockPos) {
        let mut rng = rand::rng();
        let tadpole_amount = rng.random_range(MIN_TADPOLES_SPAWN..=MAX_TADPOLES_SPAWN);

        for _ in 0..tadpole_amount {
            let x = f64::from(pos.0.x) + Self::get_random_tadpole_position_offset(&mut rng);
            let z = f64::from(pos.0.z) + Self::get_random_tadpole_position_offset(&mut rng);
            let y_rot = rng.random_range(1..=360);
            let tadpole = from_type(
                &EntityType::TADPOLE,
                Vector3::new(x, f64::from(pos.0.y) - 0.5, z),
                world,
                Uuid::new_v4(),
            );
            tadpole.get_entity().set_rotation(y_rot as f32, 0.0);
            if let Some(mob) = tadpole.get_mob() {
                mob.get_mob_entity()
                    .persistence_required
                    .store(true, Ordering::Relaxed);
            }
            world.spawn_entity(tadpole);
        }
    }

    fn get_random_tadpole_position_offset(rng: &mut impl Rng) -> f64 {
        let center = f64::from(TADPOLE_HITBOX_CENTER);
        rng.random::<f64>().clamp(center, 1.0 - center)
    }
}

impl BlockBehaviour for FrogspawnBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        args.world
            .is_some_and(|world| Self::can_survive(world, args.position))
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        args.world.schedule_block_tick(
            args.block,
            *args.position,
            Self::get_frogspawn_hatch_delay(),
            TickPriority::Normal,
        );
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if Self::can_survive(args.world, args.position) {
            args.state_id
        } else {
            Block::AIR.default_state.id
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if Self::can_survive(args.world, args.position) {
            Self::hatch_frogspawn(args.world, args.position);
        } else {
            Self::destroy_block(args.world, args.position);
        }
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if args.entity.get_entity().entity_type.id == EntityType::FALLING_BLOCK.id {
            Self::destroy_block(args.world, args.position);
        }
    }
}

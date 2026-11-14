//! `net.minecraft.world.level.pathfinder.Path`: the nodes a search found, the target it was asked
//! for and how far the mob has followed it.

use super::node::Node;
use super::path_type::PathType;
use crate::entity::Vec3;
use crate::pos::BlockPos;

/// One node of a path (the `Node` fields a path's users read: position and type; `equals` is the
/// position).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PathNode {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub ty: PathType,
}

impl PathNode {
    pub fn of(n: &Node) -> PathNode {
        PathNode { x: n.x, y: n.y, z: n.z, ty: n.ty }
    }

    pub fn pos(&self) -> BlockPos {
        BlockPos::new(self.x, self.y, self.z)
    }

    /// `Node.distanceManhattan(BlockPos)` (float).
    pub fn distance_manhattan(&self, p: BlockPos) -> f32 {
        ((p.x - self.x) as f32).abs() + ((p.y - self.y) as f32).abs() + ((p.z - self.z) as f32).abs()
    }

    /// `Node.cloneAndMove(x, y, z)` (keeps the type).
    pub fn moved(&self, x: i32, y: i32, z: i32) -> PathNode {
        PathNode { x, y, z, ty: self.ty }
    }
}

/// `Path`.
#[derive(Clone, PartialEq, Debug)]
pub struct Path {
    pub nodes: Vec<PathNode>,
    pub next_node_index: usize,
    pub target: BlockPos,
    pub dist_to_target: f32,
    pub reached: bool,
    /// The object's identity (Java compares paths by reference: `this.path != newPath`).
    pub uid: u64,
    /// (G1 Nov 14 03:33 (i)) Each node's type and malus when the search accepted the link to it, in
    /// `nodes` order (the finder's record; empty for a path built elsewhere).
    pub accepted: Vec<(PathType, f32)>,
}

impl Path {
    /// A path found by the finder, its identity `uid` drawn from its world ([`World::next_path_uid`]).
    pub fn new(nodes: Vec<PathNode>, target: BlockPos, reached: bool, uid: u64) -> Path {
        let dist_to_target = nodes.last().map(|n| n.distance_manhattan(target)).unwrap_or(f32::MAX);
        Path { nodes, next_node_index: 0, target, dist_to_target, reached, uid, accepted: Vec::new() }
    }

    pub fn advance(&mut self) {
        self.next_node_index += 1;
    }

    pub fn not_started(&self) -> bool {
        self.next_node_index == 0
    }

    pub fn is_done(&self) -> bool {
        self.next_node_index >= self.nodes.len()
    }

    pub fn end_node(&self) -> Option<PathNode> {
        self.nodes.last().copied()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// `truncateNodes(index)`.
    pub fn truncate_nodes(&mut self, index: usize) {
        if self.nodes.len() > index {
            self.nodes.truncate(index);
        }
    }

    /// `getEntityPosAtNode(entity, index)`: the node's corner plus half the mob's cell span.
    pub fn entity_pos_at_node(&self, width: f32, index: usize) -> Vec3 {
        let n = self.nodes[index];
        let half = (width + 1.0f32) as i32 as f64 * 0.5;
        Vec3::new(n.x as f64 + half, n.y as f64, n.z as f64 + half)
    }

    pub fn next_entity_pos(&self, width: f32) -> Vec3 {
        self.entity_pos_at_node(width, self.next_node_index)
    }

    pub fn next_node_pos(&self) -> BlockPos {
        self.nodes[self.next_node_index].pos()
    }

    pub fn next_node(&self) -> PathNode {
        self.nodes[self.next_node_index]
    }

    pub fn previous_node(&self) -> Option<PathNode> {
        if self.next_node_index > 0 { Some(self.nodes[self.next_node_index - 1]) } else { None }
    }

    /// `sameAs(path)`: the same node positions.
    pub fn same_as(&self, other: Option<&Path>) -> bool {
        other.is_some_and(|o| o.nodes.len() == self.nodes.len() && o.nodes.iter().zip(&self.nodes).all(|(a, b)| a.x == b.x && a.y == b.y && a.z == b.z))
    }

    /// `canReach()`.
    pub fn can_reach(&self) -> bool {
        self.reached
    }
}

#[cfg(test)]
mod uid_tests {
    use crate::block::Block;
    use crate::pos::BlockPos;
    use crate::world::World;

    /// G1 / b8949c96 (08:36): a path's identity is its world's (`World::path_uid`), not a process-wide
    /// counter's: two identical worlds, each in its own thread and run side by side with the rest of the
    /// parallel tests, find their paths with the same ids from 1 and the same paths, whatever else the process
    /// is doing; a cloned world goes on from its own count.
    #[test]
    fn path_ids_are_the_worlds_own() {
        let run = || -> Vec<(u64, usize, bool)> {
            let mut w = World::new(BlockPos::new(-8, -4, -8), [16, 8, 16]);
            let stone = Block::parse("minecraft:stone").unwrap();
            for x in -6..=6 {
                for z in -6..=6 {
                    w.set_block(BlockPos::new(x, -1, z), stone.clone(), crate::world::flags::ALL);
                }
            }
            w.mob.seed(7, 6_000);
            w.mob.ai.on = true;
            let v = crate::entity::spawn(&mut w, "minecraft:villager", [0.5, 0.0, 0.5], None).unwrap();
            for _ in 0..3 {
                w.tick();
            }
            let i = crate::entity::index_of(&w, v).unwrap();
            let before = w.path_uid.peek();
            [BlockPos::new(4, 0, 0), BlockPos::new(-4, 0, 3), BlockPos::new(0, 0, -5)]
                .into_iter()
                .map(|t| {
                    let p = crate::entity::mob::ai::with_nav(&mut w, i, |w, nm, nav, arena| nav.create_path_pos(w, nm, t, 0, arena)).expect("a path");
                    (p.uid - before, p.nodes.len(), p.reached)
                })
                .collect()
        };
        let (a, b) = std::thread::scope(|s| {
            let (ha, hb) = (s.spawn(run), s.spawn(run));
            (ha.join().unwrap(), hb.join().unwrap())
        });
        assert_eq!(a, b, "two identical worlds in two threads: the same path ids and paths");
        assert_eq!(a.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 1, 2], "consecutive ids from the world's own count");
        let w = World::new(BlockPos::new(0, 0, 0), [1, 1, 1]);
        assert_eq!((w.next_path_uid(), w.next_path_uid()), (1, 2), "a fresh world counts from 1");
        let c = w.clone();
        assert_eq!((w.next_path_uid(), c.next_path_uid()), (3, 3), "a clone goes on from its own count");
    }
}

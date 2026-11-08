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
}

static NEXT_UID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Path {
    pub fn new(nodes: Vec<PathNode>, target: BlockPos, reached: bool) -> Path {
        let dist_to_target = nodes.last().map(|n| n.distance_manhattan(target)).unwrap_or(f32::MAX);
        let uid = NEXT_UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Path { nodes, next_node_index: 0, target, dist_to_target, reached, uid }
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

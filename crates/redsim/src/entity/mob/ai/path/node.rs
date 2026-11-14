//! `Node`, `Target` and the evaluator's node map (`NodeEvaluator.nodes`), as an arena: the game
//! allocates a `Node` per cell in an `Int2ObjectOpenHashMap` keyed by `Node.createHash`; redsim
//! keeps the same nodes in a vector, found through a map on the same key. The map's iteration order
//! is never observed (the game only looks nodes up), so the arena changes nothing but the cost.

use super::path_type::PathType;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A node's index in the arena.
pub type NodeId = u32;
pub const NONE: NodeId = u32::MAX;

/// A fast hasher for integer keys (a multiply and a shift: the keys are already spread).
#[derive(Default)]
pub struct IntHasher(u64);

impl Hasher for IntHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0.rotate_left(5) ^ *b as u64).wrapping_mul(0x51_7cc1_b727_220a_95);
        }
    }
    fn write_i32(&mut self, i: i32) {
        self.0 = ((i as u32 as u64) ^ self.0).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 ^= self.0 >> 29;
    }
    fn write_u32(&mut self, i: u32) {
        self.write_i32(i as i32)
    }
    fn write_i64(&mut self, i: i64) {
        self.0 = ((i as u64) ^ self.0).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 ^= self.0 >> 29;
    }
    fn write_u64(&mut self, i: u64) {
        self.write_i64(i as i64)
    }
}

pub type IntMap<K, V> = HashMap<K, V, BuildHasherDefault<IntHasher>>;

/// `Node.createHash(x, y, z)`.
#[inline]
pub fn create_hash(x: i32, y: i32, z: i32) -> i32 {
    (y & 0xFF) | ((x & 32767) << 8) | ((z & 32767) << 24) | (if x < 0 { i32::MIN } else { 0 }) | (if z < 0 { 32768 } else { 0 })
}

/// `Mth.sqrt(float)`.
#[inline]
pub fn fsqrt(v: f32) -> f32 {
    (v as f64).sqrt() as f32
}

/// `net.minecraft.world.level.pathfinder.Node`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Node {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub hash: i32,
    pub heap_idx: i32,
    pub g: f32,
    pub h: f32,
    pub f: f32,
    pub came_from: NodeId,
    pub closed: bool,
    pub walked_distance: f32,
    pub cost_malus: f32,
    pub ty: PathType,
    /// (G1 Nov 14 03:33 (i)) The type and malus the node had when the search last accepted a link to it
    /// (its `cameFrom` set); not in 26.3's `Node`: a record for the statics.
    pub acc_ty: PathType,
    pub acc_malus: f32,
}

impl Node {
    pub fn new(x: i32, y: i32, z: i32) -> Node {
        Node { x, y, z, hash: create_hash(x, y, z), heap_idx: -1, g: 0.0, h: 0.0, f: 0.0, came_from: NONE, closed: false, walked_distance: 0.0, cost_malus: 0.0, ty: PathType::Blocked, acc_ty: PathType::Blocked, acc_malus: 0.0 }
    }

    /// `distanceTo(Node)`: the float distance.
    #[inline]
    pub fn distance_to(&self, x: i32, y: i32, z: i32) -> f32 {
        let xd = (x - self.x) as f32;
        let yd = (y - self.y) as f32;
        let zd = (z - self.z) as f32;
        fsqrt(xd * xd + yd * yd + zd * zd)
    }

    /// `distanceManhattan`.
    #[inline]
    pub fn distance_manhattan(&self, x: i32, y: i32, z: i32) -> f32 {
        let xd = ((x - self.x) as f32).abs();
        let yd = ((y - self.y) as f32).abs();
        let zd = ((z - self.z) as f32).abs();
        xd + yd + zd
    }

    pub fn in_open_set(&self) -> bool {
        self.heap_idx >= 0
    }

    pub fn pos(&self) -> crate::pos::BlockPos {
        crate::pos::BlockPos::new(self.x, self.y, self.z)
    }
}

/// `Target`: a node the search aims at, with the best node seen for it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Target {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub hash: i32,
    pub best_heuristic: f32,
    pub best_node: NodeId,
    pub reached: bool,
}

impl Target {
    pub fn new(x: i32, y: i32, z: i32) -> Target {
        Target { x, y, z, hash: create_hash(x, y, z), best_heuristic: f32::MAX, best_node: NONE, reached: false }
    }

    /// `updateBest`.
    #[inline]
    pub fn update_best(&mut self, heuristic: f32, node: NodeId) {
        if heuristic < self.best_heuristic {
            self.best_heuristic = heuristic;
            self.best_node = node;
        }
    }
}

/// The evaluator's nodes for one search (`NodeEvaluator.nodes`, cleared by `prepare`).
#[derive(Default, Clone, Debug)]
pub struct Arena {
    pub nodes: Vec<Node>,
    map: IntMap<i32, NodeId>,
}

/// Scratch buffers: nothing in them outlives a search, so any two are alike.
impl PartialEq for Arena {
    fn eq(&self, _other: &Arena) -> bool {
        true
    }
}

impl Arena {
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.map.clear();
    }

    /// `getNode(x, y, z)`: `computeIfAbsent` on the node's hash.
    #[inline]
    pub fn get(&mut self, x: i32, y: i32, z: i32) -> NodeId {
        let h = create_hash(x, y, z);
        if let Some(&id) = self.map.get(&h) {
            return id;
        }
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node::new(x, y, z));
        self.map.insert(h, id);
        id
    }

    #[inline]
    pub fn n(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    #[inline]
    pub fn n_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id as usize]
    }
}

/// `BinaryHeap` over arena nodes (its own `heapIdx` in each node), exactly as written: ties move
/// as its up/down-heap loops move them.
#[derive(Default, Clone, Debug)]
pub struct Heap {
    heap: Vec<NodeId>,
}

impl Heap {
    pub fn clear(&mut self) {
        self.heap.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn ids(&self) -> &[NodeId] {
        &self.heap
    }

    pub fn insert(&mut self, nodes: &mut [Node], id: NodeId) {
        debug_assert!(nodes[id as usize].heap_idx < 0, "OW KNOWS!");
        let idx = self.heap.len();
        self.heap.push(id);
        nodes[id as usize].heap_idx = idx as i32;
        self.up_heap(nodes, idx);
    }

    pub fn pop(&mut self, nodes: &mut [Node]) -> NodeId {
        let popped = self.heap[0];
        let last = self.heap.pop().expect("pop on an empty heap");
        if !self.heap.is_empty() {
            self.heap[0] = last;
            self.down_heap(nodes, 0);
        }
        nodes[popped as usize].heap_idx = -1;
        popped
    }

    pub fn change_cost(&mut self, nodes: &mut [Node], id: NodeId, new_cost: f32) {
        let old = nodes[id as usize].f;
        nodes[id as usize].f = new_cost;
        let idx = nodes[id as usize].heap_idx as usize;
        if new_cost < old {
            self.up_heap(nodes, idx);
        } else {
            self.down_heap(nodes, idx);
        }
    }

    fn up_heap(&mut self, nodes: &mut [Node], mut idx: usize) {
        let node = self.heap[idx];
        let cost = nodes[node as usize].f;
        while idx > 0 {
            let parent_idx = (idx - 1) >> 1;
            let parent = self.heap[parent_idx];
            if !(cost < nodes[parent as usize].f) {
                break;
            }
            self.heap[idx] = parent;
            nodes[parent as usize].heap_idx = idx as i32;
            idx = parent_idx;
        }
        self.heap[idx] = node;
        nodes[node as usize].heap_idx = idx as i32;
    }

    fn down_heap(&mut self, nodes: &mut [Node], mut idx: usize) {
        let node = self.heap[idx];
        let cost = nodes[node as usize].f;
        let size = self.heap.len();
        loop {
            let left_idx = 1 + (idx << 1);
            let right_idx = left_idx + 1;
            if left_idx >= size {
                break;
            }
            let left = self.heap[left_idx];
            let left_cost = nodes[left as usize].f;
            let (right, right_cost) = if right_idx >= size { (NONE, f32::INFINITY) } else { (self.heap[right_idx], nodes[self.heap[right_idx] as usize].f) };
            if left_cost < right_cost {
                if !(left_cost < cost) {
                    break;
                }
                self.heap[idx] = left;
                nodes[left as usize].heap_idx = idx as i32;
                idx = left_idx;
            } else {
                if !(right_cost < cost) {
                    break;
                }
                self.heap[idx] = right;
                nodes[right as usize].heap_idx = idx as i32;
                idx = right_idx;
            }
        }
        self.heap[idx] = node;
        nodes[node as usize].heap_idx = idx as i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_like_java() {
        // Node.createHash(1, 64, -3): 64 | 1 << 8 | (-3 & 32767) << 24 | 32768
        assert_eq!(create_hash(1, 64, -3), 64 | (1 << 8) | ((32765i32) << 24) | 32768);
        assert_eq!(create_hash(-1, 0, 0), (32767 << 8) | i32::MIN);
    }

    #[test]
    fn heap_pops_in_cost_order() {
        let mut a = Arena::default();
        let mut h = Heap::default();
        for (k, f) in [5.0f32, 1.0, 3.0, 2.0, 4.0].iter().enumerate() {
            let id = a.get(k as i32, 0, 0);
            a.n_mut(id).f = *f;
            h.insert(&mut a.nodes, id);
        }
        let mut out = Vec::new();
        while !h.is_empty() {
            let id = h.pop(&mut a.nodes);
            out.push(a.n(id).f);
        }
        assert_eq!(out, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }
}

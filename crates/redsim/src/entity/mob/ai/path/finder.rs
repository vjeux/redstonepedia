//! `PathFinder` (26.3): A* over the evaluator's nodes, read line by line (the open set, the node
//! budget, the reach test by Manhattan distance, the walked-distance cut, the 1.5 heuristic, the
//! best-node bookkeeping of each target, the reconstruction and the choice among targets).

use super::node::{Heap, NONE, NodeId, Target};
use super::path::{Path, PathNode};
use super::walk::WalkEval;
use crate::pos::BlockPos;

/// `PathFinder.FUDGING`.
const FUDGING: f32 = 1.5;

/// The order Java's `HashMap` / `HashSet` iterate entries with these hashes (`hashCode()`), put in
/// this order, in a table of `table` buckets: bucket by `(h ^ h >>> 16) & (table - 1)`, then the
/// order of insertion (resizes keep it). Treeified bins (8+ keys in one bucket) are not modeled.
pub fn java_hash_order(hashes: &[i32], table: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..hashes.len()).collect();
    let bucket = |h: i32| ((h ^ ((h as u32) >> 16) as i32) as u32 as usize) & (table - 1);
    idx.sort_by_key(|&k| (bucket(hashes[k]), k));
    idx
}

/// The table size of a `HashMap` that grew from the default 16 to hold `n` entries.
pub fn grown_table(n: usize) -> usize {
    let mut t = 16usize;
    while n as f64 > t as f64 * 0.75 {
        t *= 2;
    }
    t
}

/// The table size of `Sets.newHashSetWithExpectedSize(k)` (Guava `Maps.capacity`, then
/// `HashMap.tableSizeFor`).
pub fn expected_size_table(k: usize) -> usize {
    let cap = if k < 3 { k + 1 } else { (k as f32 / 0.75f32 + 1.0f32) as usize };
    cap.max(1).next_power_of_two()
}

/// `findPath(level, mob, targets, maxPathLength, reachRange, maxVisitedNodesMultiplier)`, with
/// `max_visited` = `(int)(maxVisitedNodes * multiplier)`. `targets` in the iteration order of the
/// caller's set. The evaluator was prepared (`WalkEval::new`).
pub fn find_path(eval: &mut WalkEval, targets: &[BlockPos], max_path_length: f32, reach_range: i32, max_visited: i32) -> Option<Path> {
    let from = eval.start()?;
    // Collectors.toMap: a HashMap<Target, BlockPos> (Target's hash is Node.createHash)
    let mut tos: Vec<(Target, BlockPos)> = targets.iter().map(|p| (eval.target(p.x as f64, p.y as f64, p.z as f64), *p)).collect();
    let hashes: Vec<i32> = tos.iter().map(|(t, _)| t.hash).collect();
    let order = java_hash_order(&hashes, grown_table(tos.len()));
    tos = order.iter().map(|&k| tos[k]).collect();
    search(eval, from, &mut tos, max_path_length, reach_range, max_visited)
}

fn best_h(eval: &WalkEval, id: NodeId, targets: &mut [(Target, BlockPos)]) -> f32 {
    let n = *eval.arena.n(id);
    let mut best = f32::MAX;
    for (t, _) in targets.iter_mut() {
        let h = n.distance_to(t.x, t.y, t.z);
        t.update_best(h, id);
        best = h.min(best);
    }
    best
}

/// The last search's node count when it stopped (`count`, the budget's counter) and its budget
/// (`MCTAS_PATH_DUMP`'s line).
pub static LAST_COUNT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
pub static LAST_BUDGET: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

fn search(eval: &mut WalkEval, from: NodeId, targets: &mut [(Target, BlockPos)], max_path_length: f32, reach_range: i32, max_visited: i32) -> Option<Path> {
    let r = search_inner(eval, from, targets, max_path_length, reach_range, max_visited);
    LAST_BUDGET.store(max_visited, std::sync::atomic::Ordering::Relaxed);
    r
}

fn search_inner(eval: &mut WalkEval, from: NodeId, targets: &mut [(Target, BlockPos)], max_path_length: f32, reach_range: i32, max_visited: i32) -> Option<Path> {
    let mut open = Heap::default();
    let h0 = best_h(eval, from, targets);
    {
        let f = eval.arena.n_mut(from);
        f.g = 0.0;
        f.h = h0;
        f.f = h0;
    }
    open.insert(&mut eval.arena.nodes, from);
    let mut count = 0i32;
    // the reached targets (a HashSet<Target>), in insertion order here; ordered at the end
    let mut reached: Vec<usize> = Vec::new();
    let mut neighbors: Vec<NodeId> = Vec::with_capacity(32);
    let fnode = *eval.arena.n(from);
    while !open.is_empty() {
        count += 1;
        if count >= max_visited {
            break;
        }
        let current = open.pop(&mut eval.arena.nodes);
        eval.arena.n_mut(current).closed = true;
        let c = *eval.arena.n(current);
        for (k, (t, _)) in targets.iter_mut().enumerate() {
            if c.distance_manhattan(t.x, t.y, t.z) <= reach_range as f32 {
                t.reached = true;
                if !reached.contains(&k) {
                    reached.push(k);
                }
            }
        }
        if !reached.is_empty() {
            break;
        }
        if !(c.distance_to(fnode.x, fnode.y, fnode.z) >= max_path_length) {
            eval.neighbors(&mut neighbors, current);
            for &nb in neighbors.iter() {
                let n = *eval.arena.n(nb);
                let distance = c.distance_to(n.x, n.y, n.z);
                let walked = c.walked_distance + distance;
                eval.arena.n_mut(nb).walked_distance = walked;
                let tentative = c.g + distance + n.cost_malus;
                if walked < max_path_length && (!n.in_open_set() || tentative < n.g) {
                    let h = best_h(eval, nb, targets) * FUDGING;
                    let m = eval.arena.n_mut(nb);
                    m.came_from = current;
                    m.g = tentative;
                    m.h = h;
                    let fnew = m.g + m.h;
                    if m.in_open_set() {
                        open.change_cost(&mut eval.arena.nodes, nb, fnew);
                    } else {
                        m.f = fnew;
                        open.insert(&mut eval.arena.nodes, nb);
                    }
                }
            }
        }
    }
    let paths: Vec<(usize, Path)> = if !reached.is_empty() {
        LAST_COUNT.store(count, std::sync::atomic::Ordering::Relaxed);
        // reachedTargets.stream(): HashSet<Target> order, then min by node count (the first wins ties)
        let hashes: Vec<i32> = reached.iter().map(|&k| targets[k].0.hash).collect();
        let order = java_hash_order(&hashes, expected_size_table(targets.len()));
        order.iter().map(|&j| reached[j]).map(|k| (k, reconstruct(eval, targets[k].0.best_node, targets[k].1, true))).collect()
    } else {
        LAST_COUNT.store(count, std::sync::atomic::Ordering::Relaxed);
        (0..targets.len()).map(|k| (k, reconstruct(eval, targets[k].0.best_node, targets[k].1, false))).collect()
    };
    let mut best: Option<Path> = None;
    for (_, p) in paths {
        best = match best {
            None => Some(p),
            Some(b) => {
                let better = if p.reached { p.node_count() < b.node_count() } else { p.dist_to_target < b.dist_to_target || (p.dist_to_target == b.dist_to_target && p.node_count() < b.node_count()) };
                if better { Some(p) } else { Some(b) }
            }
        };
    }
    best
}

/// `reconstructPath(closest, target, reached)`.
fn reconstruct(eval: &WalkEval, closest: NodeId, target: BlockPos, reached: bool) -> Path {
    let mut nodes = Vec::new();
    let mut id = closest;
    while id != NONE {
        let n = eval.arena.n(id);
        nodes.push(PathNode::of(n));
        id = n.came_from;
    }
    nodes.reverse();
    Path::new(nodes, target, reached)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_orders() {
        assert_eq!(grown_table(1), 16);
        assert_eq!(grown_table(12), 16);
        assert_eq!(grown_table(13), 32);
        assert_eq!(expected_size_table(1), 2);
        assert_eq!(expected_size_table(2), 4);
        assert_eq!(expected_size_table(4), 8);
        // buckets 1, 0, 1: the bucket-0 key first, then the bucket-1 keys in insertion order
        assert_eq!(java_hash_order(&[1, 16, 17], 16), vec![1, 0, 2]);
    }
}

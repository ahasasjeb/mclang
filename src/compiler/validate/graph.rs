use std::collections::{HashMap, HashSet};

/// 返回有向图中属于环的节点，使用 Kosaraju 两趟 DFS，复杂度为 O(V + E)。
///
/// `graph` 的边必须落在 `nodes` 内：集合外的前驱会被并入分量，破坏环判定。
pub(super) fn cyclic_nodes<'a>(
    nodes: impl IntoIterator<Item = &'a str>,
    graph: &HashMap<&'a str, HashSet<&'a str>>,
) -> HashSet<&'a str> {
    let nodes: Vec<&str> = nodes.into_iter().collect();
    let mut visited = HashSet::with_capacity(nodes.len());
    let mut finished = Vec::with_capacity(nodes.len());

    for &root in &nodes {
        if visited.contains(root) {
            continue;
        }
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            if expanded {
                finished.push(node);
                continue;
            }
            if !visited.insert(node) {
                continue;
            }
            stack.push((node, true));
            if let Some(neighbors) = graph.get(node) {
                for &neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        stack.push((neighbor, false));
                    }
                }
            }
        }
    }

    let mut reverse = nodes
        .iter()
        .map(|&node| (node, Vec::new()))
        .collect::<HashMap<_, _>>();
    for (&node, neighbors) in graph {
        for &neighbor in neighbors {
            if let Some(predecessors) = reverse.get_mut(neighbor) {
                predecessors.push(node);
            }
        }
    }

    visited.clear();
    let mut cyclic = HashSet::new();
    for root in finished.into_iter().rev() {
        if !visited.insert(root) {
            continue;
        }
        let mut component = vec![root];
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if let Some(predecessors) = reverse.get(node) {
                for &predecessor in predecessors {
                    if visited.insert(predecessor) {
                        component.push(predecessor);
                        stack.push(predecessor);
                    }
                }
            }
        }

        let self_loop = component.len() == 1
            && graph
                .get(root)
                .is_some_and(|neighbors| neighbors.contains(root));
        if component.len() > 1 || self_loop {
            cyclic.extend(component);
        }
    }
    cyclic
}

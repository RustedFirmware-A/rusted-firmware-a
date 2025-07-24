// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Collection of structures for describing the power domain tree.

use crate::platform::{Platform, PlatformImpl};

use super::{
    PlatformPowerState, PlatformPowerStateInterface as _, PsciCompositePowerState,
    PsciPlatformImpl, PsciPlatformInterface as _,
};
use arm_psci::{AffinityInfo, EntryPoint};
use arrayvec::ArrayVec;
use core::{
    fmt::Debug,
    ops::Range,
    slice::{Iter, IterMut},
};
use spin::mutex::{SpinMutex, SpinMutexGuard};

/// Represents a non-CPU power domain node in the power domain tree.
#[derive(Debug)]
pub struct NonCpuPowerNode {
    /// Parent node index or None if it is the top level node
    parent: Option<usize>,
    /// Local power state of the node
    local_state: PlatformPowerState,
    /// Range of descendant CPU indices
    cpu_range: Range<usize>,
    /// Requested power state of descendant CPU nodes
    requested_states: ArrayVec<PlatformPowerState, { PowerDomainTree::CPU_DOMAIN_COUNT }>,
    // OPTIMIZE: The worst case memory usage of requested_states on all NonCpuPowerNode happens
    // when the power domain tree is a complete binary tree. In this case the memory usage is
    // n^2 + n where n is CPU_DOMAIN_COUNT. The optimal case would be n * log2(n) if using Vec of
    // required capacity for each node.
}

impl NonCpuPowerNode {
    /// Create new non-CPU power node and assign its parent node index.
    pub fn new(parent: Option<usize>) -> Self {
        Self {
            parent,
            local_state: PlatformPowerState::OFF,
            cpu_range: 0..0,
            requested_states: ArrayVec::new(),
        }
    }

    /// Assign descendant CPU node index incrementally.
    fn assign_cpu(&mut self, cpu_index: usize) {
        if self.cpu_range.is_empty() {
            self.cpu_range = cpu_index..cpu_index + 1;
        } else {
            debug_assert_eq!(self.cpu_range.end, cpu_index);
            self.cpu_range.end += 1;
        }

        self.requested_states.push(PlatformPowerState::OFF);
    }

    /// Store the requested power state of a descendant CPU node.
    pub fn set_requested_power_state(&mut self, cpu_index: usize, state: PlatformPowerState) {
        assert!(self.cpu_range.contains(&cpu_index));
        self.requested_states[cpu_index - self.cpu_range.start] = state;
    }

    /// Set the local power state of the node to the lowest possible level while still meeting the
    /// power requirements of its descendant CPU nodes. This means the node cannot enter a deeper
    /// power state than the shallowest power state requested by any of its descendant CPUs.
    /// Smaller power state values represent shallower power states, therefore, it should be set to
    /// the minimal requested power state.
    pub fn set_minimal_allowed_state(&mut self) {
        self.local_state = *self.requested_states.iter().min().unwrap();
    }

    /// Get local power state of the node.
    pub fn local_state(&self) -> PlatformPowerState {
        self.local_state
    }

    /// Set local power state of the node.
    pub fn set_local_state(&mut self, local_state: PlatformPowerState) {
        self.local_state = local_state;
    }
}

/// Represents a CPU power domain node in the power domain tree.
#[derive(Debug)]
pub struct CpuPowerNode {
    /// Parent non-CPU power node index
    parent: usize,
    /// Current affinity info of the CPU
    affinity_info: AffinityInfo,
    /// Highest power level which if affected while in suspend state
    highest_affected_level: Option<usize>,
    /// Local power state of the CPU node
    local_state: PlatformPowerState,
    /// Non-secure entry point of the CPU on waking up
    entry_point: Option<EntryPoint>,
}

impl CpuPowerNode {
    pub fn new(parent: usize) -> Self {
        Self {
            parent,
            affinity_info: AffinityInfo::Off,
            highest_affected_level: None,
            local_state: PlatformPowerState::OFF,
            entry_point: None,
        }
    }

    /// Get affinity info of the CPU.
    pub fn affinity_info(&self) -> AffinityInfo {
        self.affinity_info
    }

    /// Set affinity info of the CPU.
    pub fn set_affinity_info(&mut self, affinity_info: AffinityInfo) {
        self.affinity_info = affinity_info;
    }

    /// Get highest affected power level for suspend coordination.
    pub fn highest_affected_level(&self) -> Option<usize> {
        self.highest_affected_level
    }

    /// Set highest affected power level.
    #[allow(unused)]
    pub fn set_highest_affected_level(&mut self, highest_affected_level: usize) {
        self.highest_affected_level = Some(highest_affected_level);
    }

    /// Clears highest affected power level.
    pub fn clear_highest_affected_level(&mut self) {
        self.highest_affected_level = None;
    }

    /// Get local state of the CPU.
    pub fn local_state(&self) -> PlatformPowerState {
        self.local_state
    }

    /// Set local state of the CPU.
    pub fn set_local_state(&mut self, local_state: PlatformPowerState) {
        self.local_state = local_state;
    }

    /// Store non-secure entry point of the CPU.
    pub fn set_entry_point(&mut self, entry_point: EntryPoint) {
        assert_eq!(self.entry_point, None);
        self.entry_point = Some(entry_point);
    }

    /// Get and clear stored non-secure entry point of the CPU.
    pub fn pop_entry_point(&mut self) -> Option<EntryPoint> {
        let mut entry_point = None;
        core::mem::swap(&mut self.entry_point, &mut entry_point);
        entry_point
    }
}

/// Object for locking multiple non-CPU power nodes. In order to avoid deadlocks and race
/// conditions the non-CPU power nodes are always locked from the lower level to higher.
#[derive(Debug)]
pub struct AncestorPowerDomains<'a> {
    list: ArrayVec<SpinMutexGuard<'a, NonCpuPowerNode>, { PsciPlatformImpl::MAX_POWER_LEVEL }>,
}

impl<'a> AncestorPowerDomains<'a> {
    /// Lock the selected node and its ancestors up to `max_level`.
    pub fn new_with_max_level(
        index: usize,
        max_level: usize,
        mutexes: &'a [SpinMutex<NonCpuPowerNode>],
    ) -> Self {
        let mut list = ArrayVec::new();
        let mut parent = Some(index);
        let mut level = PsciCompositePowerState::CPU_POWER_LEVEL;

        while let Some(index) = parent {
            assert!(level <= PsciPlatformImpl::MAX_POWER_LEVEL);
            if level > max_level {
                break;
            }

            let locked = mutexes[index].lock();
            parent = locked.parent;
            list.push(locked);
            level += 1;
        }

        Self { list }
    }

    /// Create immutable iterator starting from the lowest level.
    pub fn iter(&self) -> Iter<'_, SpinMutexGuard<'a, NonCpuPowerNode>> {
        self.list.iter()
    }

    /// Create mutable iterator starting from the lowest level.
    pub fn iter_mut(&mut self) -> IterMut<'_, SpinMutexGuard<'a, NonCpuPowerNode>> {
        self.list.iter_mut()
    }
}

impl Drop for AncestorPowerDomains<'_> {
    fn drop(&mut self) {
        while let Some(guard) = self.list.pop() {
            drop(guard);
        }
    }
}

/// The PowerDomainTree is responsible for storing the non-CPU and CPU power nodes and providing
/// safe ways to access for them.
pub struct PowerDomainTree {
    non_cpu_power_nodes: ArrayVec<SpinMutex<NonCpuPowerNode>, { Self::NON_CPU_DOMAIN_COUNT }>,
    cpu_power_nodes: ArrayVec<SpinMutex<CpuPowerNode>, { Self::CPU_DOMAIN_COUNT }>,
}

impl PowerDomainTree {
    const CPU_DOMAIN_COUNT: usize = PlatformImpl::CORE_COUNT;
    const NON_CPU_DOMAIN_COUNT: usize =
        PsciPlatformImpl::POWER_DOMAIN_COUNT - Self::CPU_DOMAIN_COUNT;

    /// Create power domain tree based on the BFS format topology description.
    pub fn new(topology: &[usize]) -> Self {
        // Initilize non-CPU power nodes.
        let mut non_cpu_power_nodes: ArrayVec<
            SpinMutex<NonCpuPowerNode>,
            { Self::NON_CPU_DOMAIN_COUNT },
        > = ArrayVec::new();
        let mut node_index = 0..Self::NON_CPU_DOMAIN_COUNT;
        let mut node_count: usize = 1;
        let mut parent_node_index: usize = 0;
        let mut parent_node = None;

        for _ in
            (PsciCompositePowerState::CPU_POWER_LEVEL + 1..=PsciPlatformImpl::MAX_POWER_LEVEL).rev()
        {
            let mut next_level_node_count = 0;

            for _ in 0..node_count {
                let child_count = topology[parent_node_index];

                for _ in (&mut node_index).take(child_count) {
                    non_cpu_power_nodes.push(SpinMutex::new(NonCpuPowerNode::new(parent_node)));
                }

                parent_node = Some(parent_node_index);
                next_level_node_count += child_count;
                parent_node_index += 1;
            }

            node_count = next_level_node_count;
        }

        // Check if the expected number of non-CPU nodes has been created.
        debug_assert!(node_index.is_empty());

        // Initialize CPU power nodes.
        let mut cpu_power_nodes = ArrayVec::new();
        let mut node_index = 0..Self::CPU_DOMAIN_COUNT;
        for num_children in &topology[parent_node_index..] {
            for cpu_index in (&mut node_index).take(*num_children) {
                cpu_power_nodes.push(SpinMutex::new(CpuPowerNode::new(parent_node_index - 1)));
                Self::assign_cpu(&non_cpu_power_nodes, parent_node_index - 1, cpu_index);
            }

            parent_node_index += 1;
        }

        // Check if the expected number of CPU nodes has been created.
        debug_assert!(node_index.is_empty());

        PowerDomainTree {
            non_cpu_power_nodes,
            cpu_power_nodes,
        }
    }

    /// Assigns the CPU to its ancestor non-CPU power domain node's CPU index range recursively.
    /// This can be only done when the BFS traversal reaches the CPU level.
    fn assign_cpu(
        non_cpu_power_nodes: &[SpinMutex<NonCpuPowerNode>],
        parent_index: usize,
        cpu_index: usize,
    ) {
        let mut node = non_cpu_power_nodes[parent_index].lock();
        node.assign_cpu(cpu_index);
        if let Some(parent_index) = node.parent {
            Self::assign_cpu(non_cpu_power_nodes, parent_index, cpu_index);
        }
    }

    /// Check if a given CPU is the last CPU in the system with is powered on.
    pub fn is_last_cpu(&self, cpu_index: usize) -> bool {
        self.cpu_power_nodes.iter().enumerate().all(|(index, cpu)| {
            let locked_cpu = cpu.lock();
            if index == cpu_index {
                assert_eq!(locked_cpu.affinity_info(), AffinityInfo::On);
                true
            } else {
                locked_cpu.affinity_info() == AffinityInfo::Off
            }
        })
    }

    /// Return a lock-guarded CPU node by its index.
    pub fn locked_cpu_node(&self, cpu_index: usize) -> SpinMutexGuard<CpuPowerNode> {
        self.cpu_power_nodes[cpu_index].lock()
    }

    /// Locks all ancestor nodes of a CPU, runs the closure and unlocks the nodes.
    /// This function ensures that power cordination is only possible with the proper locks
    /// acquired and it avoid deadlocks by always locking the nodes from the lowest level to the
    /// highest.
    pub fn with_ancestors_locked<F, T>(&self, cpu: &mut CpuPowerNode, f: F) -> T
    where
        F: FnOnce(&mut CpuPowerNode, AncestorPowerDomains<'_>) -> T,
    {
        self.with_ancestors_locked_to_max_level(cpu, PsciPlatformImpl::MAX_POWER_LEVEL, f)
    }

    /// Locks all ancestor nodes of a CPU up to `max_level`, runs the closure and unlocks the
    /// nodes. This function ensures that power cordination is only possible with the proper locks
    /// acquired and it avoid deadlocks by always locking the nodes from the lowest level to the
    /// highest.
    pub fn with_ancestors_locked_to_max_level<F, T>(
        &self,
        cpu: &mut CpuPowerNode,
        max_level: usize,
        f: F,
    ) -> T
    where
        F: FnOnce(&mut CpuPowerNode, AncestorPowerDomains<'_>) -> T,
    {
        let lock_list = AncestorPowerDomains::new_with_max_level(
            cpu.parent,
            max_level,
            &self.non_cpu_power_nodes,
        );
        f(cpu, lock_list)
    }
}

impl Debug for PowerDomainTree {
    /// Outputs the tree in Graphviz DOT format.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "digraph {{")?;
        for (index, ncpu) in self.non_cpu_power_nodes.iter().enumerate() {
            let nc = ncpu.lock();
            writeln!(f, "NC{} [label=\"{:#?}\"]", index, nc)?;
            if let Some(parent) = nc.parent {
                writeln!(f, "NC{} -> NC{}", parent, index)?;
            }
        }

        for (index, cpu) in self.cpu_power_nodes.iter().enumerate() {
            let c = cpu.lock();
            writeln!(f, "C{} [label=\"{:#?}\"]", index, c)?;
            writeln!(f, "NC{} -> C{}", c.parent, index)?;
        }

        writeln!(f, "}}")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::psci::{PlatformPowerStateInterface, PsciPlatformInterface};

    #[test]
    fn non_cpu_power_node() {
        let mut node = NonCpuPowerNode::new(Some(1));
        assert_eq!(node.parent, Some(1));
        assert_eq!(PlatformPowerState::OFF, node.local_state);
        assert!(node.cpu_range.is_empty());
        assert!(node.requested_states.is_empty());

        node.assign_cpu(2);
        assert_eq!(2..3, node.cpu_range);

        node.assign_cpu(3);
        assert_eq!(2..4, node.cpu_range);

        let mut requested_states = ArrayVec::new();
        requested_states.push(PlatformPowerState::OFF);
        requested_states.push(PlatformPowerState::OFF);

        assert_eq!(requested_states, node.requested_states);

        let mut requested_states = ArrayVec::new();
        requested_states.push(PlatformPowerState::OFF);
        requested_states.push(PlatformPowerState::RUN);
        node.set_requested_power_state(3, PlatformPowerState::RUN);
        assert_eq!(requested_states, node.requested_states);

        node.set_minimal_allowed_state();
        assert_eq!(PlatformPowerState::RUN, node.local_state());

        node.set_requested_power_state(3, PlatformPowerState::OFF);
        node.set_minimal_allowed_state();
        assert_eq!(PlatformPowerState::OFF, node.local_state());

        node.set_local_state(PlatformPowerState::RUN);
        assert_eq!(PlatformPowerState::RUN, node.local_state());
    }

    #[test]
    #[should_panic]
    fn non_cpu_power_node_invalid_cpu_request() {
        let mut node = NonCpuPowerNode::new(Some(1));
        node.assign_cpu(2);
        node.assign_cpu(3);
        node.set_requested_power_state(4, PlatformPowerState::RUN);
    }

    #[test]
    fn cpu_power_node() {
        let mut node = CpuPowerNode::new(3);
        assert_eq!(3, node.parent);
        assert_eq!(AffinityInfo::Off, node.affinity_info());
        assert_eq!(None, node.highest_affected_level());
        assert_eq!(PlatformPowerState::OFF, node.local_state());
        assert_eq!(None, node.pop_entry_point());

        node.set_affinity_info(AffinityInfo::On);
        assert_eq!(AffinityInfo::On, node.affinity_info());

        node.set_highest_affected_level(4);
        assert_eq!(Some(4), node.highest_affected_level());
        node.clear_highest_affected_level();
        assert_eq!(None, node.highest_affected_level());

        node.set_local_state(PlatformPowerState::RUN);
        assert_eq!(PlatformPowerState::RUN, node.local_state());

        assert_eq!(None, node.pop_entry_point());
        node.set_entry_point(EntryPoint::Entry32 {
            entry_point_address: 1,
            context_id: 2,
        });
        assert_eq!(
            Some(EntryPoint::Entry32 {
                entry_point_address: 1,
                context_id: 2
            }),
            node.pop_entry_point()
        );
        assert_eq!(None, node.pop_entry_point());
    }

    #[test]
    #[should_panic]
    fn cpu_power_node_overwrite_entry() {
        let mut node = CpuPowerNode::new(3);

        node.set_entry_point(EntryPoint::Entry32 {
            entry_point_address: 1,
            context_id: 2,
        });
        node.set_entry_point(EntryPoint::Entry32 {
            entry_point_address: 1,
            context_id: 2,
        });
    }

    #[test]
    fn power_domain_tree_create() {
        let tree = PowerDomainTree::new(PsciPlatformImpl::topology());
        let non_cpu_parents = [None, Some(0), Some(0), Some(1), Some(1), Some(2), Some(2)];
        let non_cpu_ranges = [0..13, 0..6, 6..13, 0..3, 3..6, 6..9, 9..13];
        let cpu_parents = [3, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 6];

        assert_eq!(non_cpu_parents.len(), tree.non_cpu_power_nodes.len());
        assert_eq!(cpu_parents.len(), tree.cpu_power_nodes.len());

        for ((node, parent), range) in tree
            .non_cpu_power_nodes
            .iter()
            .zip(non_cpu_parents)
            .zip(non_cpu_ranges)
        {
            assert_eq!(parent, node.lock().parent);
            assert_eq!(range, node.lock().cpu_range);
        }

        for (node, parent) in tree.cpu_power_nodes.iter().zip(cpu_parents) {
            assert_eq!(parent, node.lock().parent);
        }
    }

    #[test]
    fn power_domain_tree_is_last_cpu() {
        let tree = PowerDomainTree::new(PsciPlatformImpl::topology());

        tree.locked_cpu_node(2).set_affinity_info(AffinityInfo::On);
        assert!(tree.is_last_cpu(2));

        tree.locked_cpu_node(5).set_affinity_info(AffinityInfo::On);

        assert!(!tree.is_last_cpu(2));
    }

    #[test]
    fn power_domain_tree_with_acenstors_locked() {
        let tree = PowerDomainTree::new(PsciPlatformImpl::topology());

        let mut cpu = tree.locked_cpu_node(4);
        tree.with_ancestors_locked_to_max_level(&mut cpu, 1, |_cpu, ancestors| {
            assert_eq!(2, ancestors.iter().len());
            let mut iter = ancestors.iter();
            assert_eq!(Some(1), iter.next().unwrap().parent);
            assert_eq!(Some(0), iter.next().unwrap().parent);
        });

        let mut cpu = tree.locked_cpu_node(12);
        tree.with_ancestors_locked(&mut cpu, |_cpu, mut ancestors| {
            assert_eq!(3, ancestors.iter().len());
            let mut iter = ancestors.iter_mut();
            assert_eq!(Some(2), iter.next().unwrap().parent);
            assert_eq!(Some(0), iter.next().unwrap().parent);
            assert_eq!(None, iter.next().unwrap().parent);
        });
    }
}

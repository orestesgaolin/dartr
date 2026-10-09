// Dart source: package:collection 1.19.1 lib/src/priority_queue.dart (HeapPriorityQueue)

//! The solver explores solutions in the order of a `HeapPriorityQueue`. The
//! comparison of solutions is not a total order (two different solutions can
//! compare equal), so the order in which equal solutions are dequeued
//! depends on the exact heap algorithm. This is a port of that algorithm so
//! that the solver picks the same solution as Dart.

use std::cmp::Ordering;

/// Dart `HeapPriorityQueue`: a binary heap stored in a list. The comparison
/// is passed to each operation.
pub struct HeapPriorityQueue<E> {
    queue: Vec<E>,
}

impl<E> Default for HeapPriorityQueue<E> {
    fn default() -> Self {
        HeapPriorityQueue { queue: Vec::new() }
    }
}

impl<E> HeapPriorityQueue<E> {
    pub fn new() -> Self {
        HeapPriorityQueue { queue: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Dart `add`: `_bubbleUp(element, _length++)`.
    pub fn add(&mut self, element: E, comparison: &mut impl FnMut(&E, &E) -> Ordering) {
        let index = self.queue.len();
        self.queue.push(element);
        self.bubble_up(index, comparison);
    }

    /// Dart `removeFirst`.
    pub fn remove_first(&mut self, comparison: &mut impl FnMut(&E, &E) -> Ordering) -> E {
        assert!(!self.queue.is_empty(), "No element");
        // Dart: result = [0]; last = _removeLast(); if (_length > 0)
        // _bubbleDown(last, 0). Moving the last element to index 0 and then
        // bubbling it down is the same.
        let last_index = self.queue.len() - 1;
        self.queue.swap(0, last_index);
        let result = self.queue.pop().unwrap();
        if !self.queue.is_empty() {
            self.bubble_down(0, comparison);
        }
        result
    }

    /// Dart `_bubbleUp(element, index)` where the element is at [index].
    fn bubble_up(&mut self, mut index: usize, comparison: &mut impl FnMut(&E, &E) -> Ordering) {
        while index > 0 {
            let parent_index = (index - 1) / 2;
            if comparison(&self.queue[index], &self.queue[parent_index]) == Ordering::Greater {
                break;
            }
            self.queue.swap(index, parent_index);
            index = parent_index;
        }
    }

    /// Dart `_bubbleDown(element, index)` where the element is at [index].
    fn bubble_down(&mut self, mut index: usize, comparison: &mut impl FnMut(&E, &E) -> Ordering) {
        let length = self.queue.len();
        let mut right_child_index = index * 2 + 2;
        while right_child_index < length {
            let left_child_index = right_child_index - 1;
            let comp = comparison(&self.queue[left_child_index], &self.queue[right_child_index]);
            let min_child_index = if comp == Ordering::Less {
                left_child_index
            } else {
                right_child_index
            };
            let comp = comparison(&self.queue[index], &self.queue[min_child_index]);
            if comp != Ordering::Greater {
                return;
            }
            self.queue.swap(index, min_child_index);
            index = min_child_index;
            right_child_index = index * 2 + 2;
        }
        let left_child_index = right_child_index - 1;
        if left_child_index < length {
            let comp = comparison(&self.queue[index], &self.queue[left_child_index]);
            if comp == Ordering::Greater {
                self.queue.swap(index, left_child_index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_elements() {
        let mut queue = HeapPriorityQueue::new();
        let mut cmp = |a: &i32, b: &i32| a.cmp(b);
        for value in [5, 3, 8, 1, 9, 2, 7] {
            queue.add(value, &mut cmp);
        }
        let mut result = Vec::new();
        while !queue.is_empty() {
            result.push(queue.remove_first(&mut cmp));
        }
        assert_eq!(result, vec![1, 2, 3, 5, 7, 8, 9]);
    }
}

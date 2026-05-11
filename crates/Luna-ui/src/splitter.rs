use crate::pane::PaneId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug)]
pub struct PaneRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Info for a draggable divider.
#[derive(Clone, Copy, Debug)]
pub struct DividerInfo {
    pub hitbox: PaneRect,
    pub direction: SplitDirection,
    pub pane_id: PaneId,
    pub parent_rect: PaneRect,
}

pub enum PaneTree {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<PaneTree>,
        second: Box<PaneTree>,
    },
}

impl PaneTree {
    pub fn leaf(id: PaneId) -> Self {
        PaneTree::Leaf(id)
    }

    pub fn all_panes(&self) -> Vec<PaneId> {
        match self {
            PaneTree::Leaf(id) => vec![*id],
            PaneTree::Split { first, second, .. } => {
                let mut result = first.all_panes();
                result.extend(second.all_panes());
                result
            }
        }
    }

    pub fn find_active(&self, active: PaneId) -> Option<PaneId> {
        let panes = self.all_panes();
        if panes.contains(&active) {
            Some(active)
        } else {
            panes.into_iter().next()
        }
    }

    /// Split a leaf pane into two panes, returning both IDs.
    /// Takes ownership via swap internally, avoiding borrow issues.
    #[allow(clippy::result_unit_err)]
    pub fn split(
        &mut self,
        pane_id: PaneId,
        new_id: PaneId,
        direction: SplitDirection,
    ) -> Result<(PaneId, PaneId), ()> {
        let mut temp = PaneTree::Leaf(PaneId(u64::MAX));
        std::mem::swap(self, &mut temp);
        let (new_tree, result) = temp.split_into(pane_id, new_id, direction);
        *self = new_tree;
        result
    }

    fn split_into(
        self,
        pane_id: PaneId,
        new_id: PaneId,
        direction: SplitDirection,
    ) -> (PaneTree, Result<(PaneId, PaneId), ()>) {
        match self {
            PaneTree::Leaf(id) => {
                if id == pane_id {
                    let new_tree = PaneTree::Split {
                        direction,
                        ratio: 0.5,
                        first: Box::new(PaneTree::Leaf(pane_id)),
                        second: Box::new(PaneTree::Leaf(new_id)),
                    };
                    (new_tree, Ok((pane_id, new_id)))
                } else {
                    (PaneTree::Leaf(id), Err(()))
                }
            }
            PaneTree::Split {
                direction: dir,
                ratio,
                first,
                second,
            } => {
                let (new_first, result) = first.split_into(pane_id, new_id, direction);
                if result.is_ok() {
                    return (
                        PaneTree::Split {
                            direction: dir,
                            ratio,
                            first: Box::new(new_first),
                            second,
                        },
                        result,
                    );
                }
                let (new_second, result) = second.split_into(pane_id, new_id, direction);
                (
                    PaneTree::Split {
                        direction: dir,
                        ratio,
                        first: Box::new(new_first),
                        second: Box::new(new_second),
                    },
                    result,
                )
            }
        }
    }

    /// Close a pane and collapse the tree. Returns the removed PaneId.
    /// Won't close the last remaining pane.
    pub fn close(&mut self, pane_id: PaneId) -> Option<PaneId> {
        if self.all_panes().len() <= 1 {
            return None;
        }
        let mut temp = PaneTree::Leaf(PaneId(u64::MAX));
        std::mem::swap(self, &mut temp);
        let (new_tree, removed) = temp.remove(pane_id);
        *self = new_tree;
        removed
    }

    fn remove(self, pane_id: PaneId) -> (PaneTree, Option<PaneId>) {
        match self {
            PaneTree::Leaf(id) => (PaneTree::Leaf(id), Some(id)),
            PaneTree::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // If the target is a direct child leaf, collapse this split
                if matches!(&*first, PaneTree::Leaf(id) if *id == pane_id) {
                    return (*second, Some(pane_id));
                }
                if matches!(&*second, PaneTree::Leaf(id) if *id == pane_id) {
                    return (*first, Some(pane_id));
                }

                // Recurse into first
                let (new_first, result) = first.remove(pane_id);
                if result.is_some() {
                    return (
                        PaneTree::Split {
                            direction,
                            ratio,
                            first: Box::new(new_first),
                            second,
                        },
                        result,
                    );
                }

                // Recurse into second
                let (new_second, result) = second.remove(pane_id);
                (
                    PaneTree::Split {
                        direction,
                        ratio,
                        first: Box::new(new_first),
                        second: Box::new(new_second),
                    },
                    result,
                )
            }
        }
    }

    /// Compute layout rectangles for all leaf panes within the given bounding rect.
    pub fn get_layout(&self, rect: PaneRect) -> Vec<(PaneId, PaneRect)> {
        let mut result = Vec::new();
        self.layout_rects(rect, &mut result);
        result
    }

    fn layout_rects(&self, rect: PaneRect, out: &mut Vec<(PaneId, PaneRect)>) {
        match self {
            PaneTree::Leaf(id) => {
                out.push((*id, rect));
            }
            PaneTree::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (r1, r2) = match direction {
                    SplitDirection::Horizontal => {
                        let h1 = (rect.h * ratio).max(0.0);
                        (
                            PaneRect {
                                x: rect.x,
                                y: rect.y,
                                w: rect.w,
                                h: h1,
                            },
                            PaneRect {
                                x: rect.x,
                                y: rect.y + h1,
                                w: rect.w,
                                h: (rect.h - h1).max(0.0),
                            },
                        )
                    }
                    SplitDirection::Vertical => {
                        let w1 = (rect.w * ratio).max(0.0);
                        (
                            PaneRect {
                                x: rect.x,
                                y: rect.y,
                                w: w1,
                                h: rect.h,
                            },
                            PaneRect {
                                x: rect.x + w1,
                                y: rect.y,
                                w: (rect.w - w1).max(0.0),
                                h: rect.h,
                            },
                        )
                    }
                };
                first.layout_rects(r1, out);
                second.layout_rects(r2, out);
            }
        }
    }

    /// Compute divider hitboxes with direction and the first-branch pane id.
    pub fn get_dividers(&self, rect: PaneRect) -> Vec<DividerInfo> {
        let mut result = Vec::new();
        self.dividers(rect, rect, &mut result);
        result
    }

    fn dividers(&self, rect: PaneRect, parent: PaneRect, out: &mut Vec<DividerInfo>) {
        match self {
            PaneTree::Leaf(_) => {}
            PaneTree::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let first_pane = first.all_panes().first().copied().unwrap_or(PaneId(0));
                let hitbox = match direction {
                    SplitDirection::Horizontal => {
                        let h1 = (rect.h * ratio).max(0.0);
                        PaneRect {
                            x: rect.x,
                            y: rect.y + h1 - 2.0,
                            w: rect.w,
                            h: 6.0,
                        }
                    }
                    SplitDirection::Vertical => {
                        let w1 = (rect.w * ratio).max(0.0);
                        PaneRect {
                            x: rect.x + w1 - 2.0,
                            y: rect.y,
                            w: 6.0,
                            h: rect.h,
                        }
                    }
                };
                out.push(DividerInfo {
                    hitbox,
                    direction: *direction,
                    pane_id: first_pane,
                    parent_rect: parent,
                });

                match direction {
                    SplitDirection::Horizontal => {
                        let h1 = (rect.h * ratio).max(0.0);
                        first.dividers(
                            PaneRect {
                                x: rect.x,
                                y: rect.y,
                                w: rect.w,
                                h: h1,
                            },
                            rect,
                            out,
                        );
                        second.dividers(
                            PaneRect {
                                x: rect.x,
                                y: rect.y + h1,
                                w: rect.w,
                                h: rect.h - h1,
                            },
                            rect,
                            out,
                        );
                    }
                    SplitDirection::Vertical => {
                        let w1 = (rect.w * ratio).max(0.0);
                        first.dividers(
                            PaneRect {
                                x: rect.x,
                                y: rect.y,
                                w: w1,
                                h: rect.h,
                            },
                            rect,
                            out,
                        );
                        second.dividers(
                            PaneRect {
                                x: rect.x + w1,
                                y: rect.y,
                                w: rect.w - w1,
                                h: rect.h,
                            },
                            rect,
                            out,
                        );
                    }
                }
            }
        }
    }

    /// Update the ratio of the split that contains the given pane_id.
    pub fn set_ratio(&mut self, pane_id: PaneId, ratio: f32) {
        match self {
            PaneTree::Leaf(_) => {}
            PaneTree::Split { first, second, .. } => {
                // Check if this split's children contain the pane_id
                let first_panes = first.all_panes();
                let second_panes = second.all_panes();
                if first_panes.contains(&pane_id) {
                    if let PaneTree::Split { ratio: r, .. } = self {
                        *r = ratio.clamp(0.1, 0.9);
                    }
                } else if second_panes.contains(&pane_id) {
                    if let PaneTree::Split { ratio: r, .. } = self {
                        *r = (1.0 - ratio).clamp(0.1, 0.9);
                    }
                } else {
                    first.set_ratio(pane_id, ratio);
                    second.set_ratio(pane_id, ratio);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_leaf() {
        let mut tree = PaneTree::Leaf(PaneId(1));
        let result = tree.split(PaneId(1), PaneId(2), SplitDirection::Vertical);
        assert!(result.is_ok());
        let (a, b) = result.unwrap();
        assert_eq!(a, PaneId(1));
        assert_eq!(b, PaneId(2));
        assert_eq!(tree.all_panes().len(), 2);
        assert!(matches!(tree, PaneTree::Split { .. }));
    }

    #[test]
    fn test_split_wrong_id() {
        let mut tree = PaneTree::Leaf(PaneId(1));
        let result = tree.split(PaneId(99), PaneId(2), SplitDirection::Vertical);
        assert!(result.is_err());
    }

    #[test]
    fn test_close_pane() {
        let mut tree = PaneTree::Leaf(PaneId(1));
        let result = tree
            .split(PaneId(1), PaneId(2), SplitDirection::Vertical)
            .unwrap();
        assert_eq!(result, (PaneId(1), PaneId(2)));

        // Close one pane -> should collapse back to single leaf
        let removed = tree.close(PaneId(2));
        assert_eq!(removed, Some(PaneId(2)));
        assert_eq!(tree.all_panes(), vec![PaneId(1)]);
        assert!(matches!(tree, PaneTree::Leaf(_)));
    }

    #[test]
    fn test_close_last_pane_fails() {
        let mut tree = PaneTree::Leaf(PaneId(1));
        let removed = tree.close(PaneId(1));
        assert_eq!(removed, None);
        assert_eq!(tree.all_panes(), vec![PaneId(1)]);
    }

    #[test]
    fn test_four_pane_layout_no_overlap() {
        let mut tree = PaneTree::Leaf(PaneId(1));
        tree.split(PaneId(1), PaneId(2), SplitDirection::Vertical)
            .unwrap();
        tree.split(PaneId(1), PaneId(3), SplitDirection::Horizontal)
            .unwrap();
        tree.split(PaneId(2), PaneId(4), SplitDirection::Horizontal)
            .unwrap();

        let panes = tree.all_panes();
        assert_eq!(panes.len(), 4);
        // Order depends on tree structure; just verify 4 unique IDs
        let mut ids: Vec<_> = panes;
        ids.sort();
        assert_eq!(ids, vec![PaneId(1), PaneId(2), PaneId(3), PaneId(4)]);

        let rect = PaneRect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        let layouts = tree.get_layout(rect);
        assert_eq!(layouts.len(), 4);

        // Verify no overlap: every pair of rects should not intersect
        for i in 0..layouts.len() {
            for j in (i + 1)..layouts.len() {
                let a = layouts[i].1;
                let b = layouts[j].1;
                let overlap_x = a.x < b.x + b.w && a.x + a.w > b.x;
                let overlap_y = a.y < b.y + b.h && a.y + a.h > b.y;
                assert!(
                    !(overlap_x && overlap_y),
                    "Panes {} and {} overlap: {:?} vs {:?}",
                    layouts[i].0 .0,
                    layouts[j].0 .0,
                    a,
                    b
                );
            }
        }

        // Sum of areas should approximately equal total area (minus divider space)
        let total_area: f32 = layouts.iter().map(|(_, r)| r.w * r.h).sum();
        let bounding_area = rect.w * rect.h;
        assert!(
            (total_area - bounding_area).abs() < 0.01,
            "Total area {} should equal bounding area {}",
            total_area,
            bounding_area
        );
    }

    #[test]
    fn test_split_recursive_in_deep_tree() {
        // Build: split 1->(1,2 v), split 1->(1,3 h), split 3->(3,5 h)
        let mut tree = PaneTree::Leaf(PaneId(1));
        tree.split(PaneId(1), PaneId(2), SplitDirection::Vertical)
            .unwrap();
        tree.split(PaneId(1), PaneId(3), SplitDirection::Horizontal)
            .unwrap();
        // Now split the rightmost leaf (pane 3 is inside first's subtree)
        tree.split(PaneId(3), PaneId(5), SplitDirection::Horizontal)
            .unwrap();

        assert_eq!(tree.all_panes().len(), 4);

        let rect = PaneRect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let layouts = tree.get_layout(rect);
        assert_eq!(layouts.len(), 4);
    }
}

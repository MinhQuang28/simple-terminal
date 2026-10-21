use super::*;

fn ids(n: usize) -> Vec<Uuid> {
    (0..n).map(|_| Uuid::new_v4()).collect()
}

#[test]
fn splits_nest_and_join_same_direction() {
    let [a, b, c] = ids(3)[..] else { unreachable!() };
    let mut layout = PaneLayout::Leaf(a);
    assert!(layout.split(a, SplitDir::Right, b));
    assert!(layout.split(b, SplitDir::Down, c));
    assert_eq!(layout.leaves(), vec![a, b, c]);
    let rects = layout.rects(Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0)), 0.0);
    assert_eq!(rects[0].1.width(), 50.0);
    assert_eq!(rects[2].1.top(), 50.0);
    assert_eq!(layout.neighbor(a, Nav::Right), Some(b));
    assert_eq!(layout.neighbor(c, Nav::Up), Some(b));
    assert_eq!(layout.neighbor(a, Nav::Left), None);
}

#[test]
fn removing_collapses_single_child_splits() {
    let [a, b, c] = ids(3)[..] else { unreachable!() };
    let mut layout = PaneLayout::from_flat(&[a, b, c], SplitDir::Right);
    assert!(layout.remove(b));
    assert!(layout.remove(c));
    assert_eq!(layout, PaneLayout::Leaf(a));
}

#[test]
fn resize_keeps_a_minimum_share() {
    let [a, b] = ids(2)[..] else { unreachable!() };
    let mut layout = PaneLayout::from_flat(&[a, b], SplitDir::Right);
    layout.resize(&[], 0, 10.0);
    let PaneLayout::Split { sizes, .. } = &layout else { panic!() };
    assert!((sizes[1] - 0.1).abs() < 1e-5);
}

//! Example: restrict borrows to disjoint fields and avoid cloning the whole map.
//! Compile/check locally (if rustc installed): rustc --edition=2021 --test split_state.rs -o /tmp/split_state_test && /tmp/split_state_test

struct Map { blocked: Vec<bool> }
struct Unit { pos: usize, steps: usize }

struct Game {
    map: Map,
    units: Vec<Unit>,
}

// No need for `&mut Game`, so the caller can borrow `map` and `units` separately.
fn step_units(map: &Map, units: &mut [Unit]) {
    for unit in units {
        if let Some(false) = map.blocked.get(unit.pos + 1).copied() {
            unit.pos += 1;
            unit.steps += 1;
        }
    }
}

impl Game {
    fn tick(&mut self) {
        let (map, units) = (&self.map, &mut self.units);
        step_units(map, units);
    }
}

#[test]
fn tick_changes_original_state_without_map_clone() {
    let mut game = Game {
        map: Map { blocked: vec![false, false, true] },
        units: vec![Unit { pos: 0, steps: 0 }],
    };
    game.tick();
    assert_eq!(game.units[0].pos, 1);
    assert_eq!(game.units[0].steps, 1);
    game.tick();
    assert_eq!(game.units[0].pos, 1); // blocked next tile
}

//! Fixture: marked as a hot path with `--hot 'src/hot*'` in tests.

pub fn step(frames: &[Vec<f32>]) -> f32 {
    let mut total = 0.0;
    for f in frames {
        let copy: Vec<f32> = f.iter().copied().collect();
        total += copy.iter().sum::<f32>();
    }
    total
}

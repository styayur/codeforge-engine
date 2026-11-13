pub fn first(values: &[i32]) -> i32 {
    values.first().copied().unwrap()
}

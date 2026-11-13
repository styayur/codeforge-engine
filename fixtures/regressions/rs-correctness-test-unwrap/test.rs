#[cfg(test)]
mod tests {
    #[test]
    fn result_is_available() {
        let value: Option<i32> = Some(1);
        let actual = value.unwrap();
        assert_eq!(actual, 1);
    }
}

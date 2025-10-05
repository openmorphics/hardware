pub fn stub() -> &'static str { "ok" }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stub_ok() {
        assert_eq!(stub(), "ok");
    }
}

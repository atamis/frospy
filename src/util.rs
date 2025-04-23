use rand;
use rand::Rng;
use std::iter;

pub fn random_name_tag(tag: &str, n: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::rng();
    let one_char = || CHARSET[rng.random_range(0..CHARSET.len())] as char;
    let mut s = String::with_capacity(tag.len() + n);
    s.push_str(tag);
    s.extend(iter::repeat_with(one_char).take(n));
    s
}

pub fn random_name() -> String {
    random_name_tag("", 10)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_random_name() {
        assert!(random_name().len() == 10);
    }

    #[test]
    fn test_random_name_tag() {
        assert!(random_name_tag("", 5).len() == 5);
        assert!(random_name_tag("tag_", 5).len() == 9);
        assert!(random_name_tag("tag_", 5).starts_with("tag_"));
    }
}

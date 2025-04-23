use super::header::*;

#[test]
fn test_env() {
    let mut env = ListEnv::new();
    assert_eq!(None, env.get("test"));
    env.insert("test".to_string(), Value::Integer(1));
    assert_eq!(Some(Value::Integer(1)), env.get("test"));
}

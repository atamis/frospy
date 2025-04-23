// Copyright (c) 2025 Azrea Amis

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::mem;

// REMOVE
#[derive(Debug, Clone, PartialEq)]
pub enum ThunkRef {}
// ENDREMOVE

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Atom(String),
    Thunk { env: Env, fp: ThunkRef },
    BuiltIn(BuiltinFp),
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Value::*;
        match self {
            Integer(i) => f.write_fmt(format_args!("{i}")),
            Atom(a) => f.write_fmt(format_args!("'{a}")),
            Thunk { fp, .. } => f.write_fmt(format_args!("&{fp:?}")),
            BuiltIn(fp) => f.write_fmt(format_args!("&{fp:?}")),
        }
    }
}

impl Value {
    pub fn get_name(&self) -> Option<&str> {
        match self {
            Value::Integer(_) => None,
            Value::Atom(s) => Some(&s),
            Value::Thunk { .. } => None,
            Value::BuiltIn(_) => None,
        }
    }

    pub fn get_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            Value::Atom(_) => None,
            Value::Thunk { .. } => None,
            Value::BuiltIn(_) => None,
        }
    }

    fn is_builtin(&self) -> bool {
        match self {
            Value::BuiltIn(_) => true,
            _ => false,
        }
    }
}

// type Env = HashMap<String, Value>;
type Env = ListEnv;

type Stack = Vec<Value>;

// type Fp = fn(Env, &mut Stack);
type BuiltinFp = fn(&mut Env, &mut Stack);

mod list {
    // https://rust-unofficial.github.io/too-many-lists/third-final.html

    // Copyright (c) 2015 Aria Desires

    // Permission is hereby granted, free of charge, to any person obtaining a copy
    // of this software and associated documentation files (the "Software"), to deal
    // in the Software without restriction, including without limitation the rights
    // to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
    // copies of the Software, and to permit persons to whom the Software is
    // furnished to do so, subject to the following conditions:

    // The above copyright notice and this permission notice shall be included in
    // all copies or substantial portions of the Software.

    // THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
    // IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
    // FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
    // AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
    // LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
    // OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
    // THE SOFTWARE.

    use std::rc::Rc;

    #[derive(Debug)]
    pub struct List<T> {
        head: Link<T>,
    }

    type Link<T> = Option<Rc<Node<T>>>;

    #[derive(Debug)]
    struct Node<T> {
        elem: T,
        next: Link<T>,
    }

    impl<T> List<T> {
        pub fn new() -> Self {
            List { head: None }
        }

        pub fn prepend(&self, elem: T) -> List<T> {
            List {
                head: Some(Rc::new(Node {
                    elem,
                    next: self.head.clone(),
                })),
            }
        }

        pub fn tail(&self) -> List<T> {
            List {
                head: self.head.as_ref().and_then(|node| node.next.clone()),
            }
        }

        pub fn head(&self) -> Option<&T> {
            self.head.as_ref().map(|node| &node.elem)
        }

        pub fn iter(&self) -> Iter<'_, T> {
            Iter {
                next: self.head.as_deref(),
            }
        }

        // MIT License, Copyright (c) 2025 Azrea Amis and contributors

        pub fn is_empty(&self) -> bool {
            self.head.is_none()
        }

        pub fn ptr_copy(&self) -> List<T> {
            if self.is_empty() {
                List::new()
            } else {
                List {
                    head: Some(Rc::clone(&self.head.as_ref().unwrap())),
                }
            }
        }

        pub fn same_list(&self, l: &List<T>) -> bool {
            (self.is_empty() && l.is_empty())
                || (Rc::ptr_eq(&self.head.as_ref().unwrap(), &l.head.as_ref().unwrap()))
        }

        pub fn find_map<B>(&self, f: impl Fn(&T) -> Option<B>) -> Option<B> {
            if self.is_empty() {
                None
            } else {
                let h = self.head().unwrap();

                if let Some(v) = f(h) {
                    Some(v)
                } else {
                    self.tail().find_map(f)
                }
            }
        }
    }

    impl<T: Clone> List<T> {
        pub fn filter_first(&self, f: impl Fn(&T) -> bool) -> List<T> {
            if self.is_empty() {
                List::new()
            } else {
                let h = self.head().unwrap();
                let t = self.tail();

                if f(h) {
                    t
                } else {
                    let r = t.filter_first(f);

                    if t.same_list(&r) {
                        self.ptr_copy()
                    } else {
                        t.prepend((*h).clone())
                    }
                }
            }
        }
    }

    // End Copyright (c) 2025 Azrea Amis
    // Start of Copyright (c) 2015 Aria Desires

    impl<T> Drop for List<T> {
        fn drop(&mut self) {
            let mut head = self.head.take();
            while let Some(node) = head {
                if let Ok(mut node) = Rc::try_unwrap(node) {
                    head = node.next.take();
                } else {
                    break;
                }
            }
        }
    }

    pub struct Iter<'a, T> {
        next: Option<&'a Node<T>>,
    }

    impl<'a, T> Iterator for Iter<'a, T> {
        type Item = &'a T;

        fn next(&mut self) -> Option<Self::Item> {
            self.next.map(|node| {
                self.next = node.next.as_deref();
                &node.elem
            })
        }
    }
}

pub struct ListEnv(list::List<(String, Value)>);

impl ListEnv {
    pub fn new() -> Self {
        ListEnv(list::List::new())
    }

    pub fn insert(&mut self, key: String, val: Value) {
        let mut l = mem::replace(&mut self.0, list::List::new());

        l = l.filter_first(|v| v.0 == key);

        l = l.prepend((key, val));

        let _ = mem::replace(&mut self.0, l);
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.0
            .find_map(|(k, v)| if key == k { Some(v.clone()) } else { None })
    }
}

impl PartialEq for ListEnv {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_list(&other.0)
    }
}

impl Clone for ListEnv {
    fn clone(&self) -> Self {
        Self(self.0.ptr_copy())
    }
}

impl Debug for ListEnv {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str("ListEnv(")?;

        for i in self.0.iter() {
            if !i.1.is_builtin() {
                f.write_fmt(format_args!("{:?}, ", i))?;
            }
        }

        f.write_str(")")
    }
}

// #[inline(always)]
// fn call_value(env: &mut Env, stack: &mut Stack, v: Value) {
//     match v {
//         Value::Thunk { env, fp } => fp(env.clone(), stack),
//         Value::BuiltIn(fp) => fp(env, stack),
//         _ => panic!("Can't force value {:?}", v),
//     }
// }

// fn builtin_force(env: &mut Env, stack: &mut Stack) {
//     let t = stack.pop().expect("Stack empty").clone();
//     call_value(env, stack, t);

//     // if let Value::Thunk { env, fp } = t {
//     //     fp(env, stack)
//     // } else {
//     //     panic!("Can't force non-thunk {:?}", t);
//     // }
// }

pub fn builtin_pop(env: &mut Env, stack: &mut Stack) {
    let name = stack
        .pop()
        .expect("Stack empty")
        .get_name()
        .expect("Not a name")
        .to_string();

    let value = stack.pop().expect("Stack empty");

    env.insert(name, value);
}

pub fn builtin_push(env: &mut Env, stack: &mut Stack) {
    let name = stack
        .pop()
        .expect("Stack empty")
        .get_name()
        .expect("Not a name")
        .to_string();

    let value = env
        .get(&name)
        .expect(&format!("Unbound name {name}"))
        .clone();

    stack.push(value);
}

pub fn builtin_inc(_env: &mut Env, stack: &mut Stack) {
    let v = stack.pop().expect("Stack empty");

    let n = if let Some(i) = v.get_integer() {
        i
    } else {
        panic!("Not integer: {:?}", v)
    };

    stack.push(Value::Integer(n + 1))
}

pub fn builtin_println(_env: &mut Env, stack: &mut Stack) {
    let v = stack.pop().expect("Stack empty");

    println!("{v}");

    // let mut s = String::new();
    // std::io::stdin()
    //     .read_line(&mut s)
    //     .expect("Error waiting for user input");
}

pub fn make_env() -> Env {
    let mut env = Env::new();

    env.insert("pop".to_string(), Value::BuiltIn(builtin_pop));
    env.insert("push".to_string(), Value::BuiltIn(builtin_push));
    env.insert("inc".to_string(), Value::BuiltIn(builtin_inc));
    env.insert("println".to_string(), Value::BuiltIn(builtin_println));

    env
}

pub struct Frame {
    tr: ThunkRef,
    env: Env,
}

pub fn builtin_force_cc(stack: &mut Stack, cur_frame: &mut Frame) -> Frame {
    let cc = stack.pop().unwrap();
    let th = stack.pop().unwrap();
    match th {
        Value::Thunk { env, fp } => {
            stack.push(cc);
            let nf = Frame {
                env: env.clone(),
                tr: fp,
            };
            return nf;
            // cs.push(nf);
        }
        Value::BuiltIn(f) => {
            f(&mut cur_frame.env, stack);
            if let Value::Thunk {
                env: cc_env,
                fp: cc_fp,
            } = cc
            {
                let nf = Frame {
                    env: cc_env.clone(),
                    tr: cc_fp,
                };
                return nf;
                // cs.push(nf);
            } else {
                panic!(
                    "Forcing builtin with CC, expected CC to be thunk, got {:?}",
                    cc
                )
            }
        }
        x => panic!("Can't force non-thunk: {:?}", x),
    }
}

pub fn builtin_force_cc_bare(stack: &mut Stack) -> Frame {
    let cc = stack.pop().unwrap();
    match cc {
        Value::Thunk { env, fp } => {
            let nf = Frame {
                env: env.clone(),
                tr: fp,
            };
            return nf;
        }
        x => panic!("Can't force non-thunk: {:?}", x),
    }
}

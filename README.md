# clone-on-capture

This crate provides a macro that makes all captures clone automatically.

## Use case

Given this code snippet:

```rust
fn foo() {
    let a = "a".to_string();
    let multiply_by_a = move || {
        println!("{a:?}");
    };
    println!("{a:?}");
}
```

You will get an error that `a` was moved.
To fix it you can clone `a` in a temporary scope:

```rust
fn foo() {
    let a = "a".to_string();
    let multiply_by_a = {
        let a = a.clone();
        move || {
            println!("{a:?}");
        }
    };
    println!("{a:?}");
}
```

Cloning can get tedious, `clone-on-capture` macro can automatically generate that code for you:

```rust
#[clone_on_capture]
fn foo() {
    let a = "a".to_string();
    let multiply_by_a = move || {
        println!("{a:?}");
    };
    println!("{a:?}");
}
```

This will also clone variables that implement `Copy`, but it is not a problem as `.clone()` is just an explicit way to do the same as `Copy`.
https://doc.rust-lang.org/std/marker/trait.Copy.html#whats-the-difference-between-copy-and-clone

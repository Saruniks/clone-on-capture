use clone_on_capture::clone_on_capture;

#[test]
#[clone_on_capture]
fn check_addresses() {
    let a = "a".to_string();
    let a_address = a.as_ptr();

    let closure = move || {
        let b = a;
        let b_address = b.as_ptr();
        assert_ne!(a_address, b_address);
    };

    closure();
}

#[test]
#[clone_on_capture]
fn do_not_clone_prefix() {
    let dc_a = "a".to_string();
    let dc_a_address = dc_a.as_ptr();

    let closure = move || {
        let b = dc_a;
        let b_address = b.as_ptr();
        assert_eq!(dc_a_address, b_address);
    };

    closure();
}

#[test]
#[clone_on_capture]
fn simple_closure_with_format() {
    let a = "a".to_string();
    let _closure = move || {
        format!("{}", a);
    };
    format!("{}", a);
}

#[test]
#[clone_on_capture]
fn simple_closure_with_assign() {
    let a = "a".to_string();
    let _closure = move || {
        let _b = a;
    };
    let _c = a;
}

#[test]
#[clone_on_capture]
fn none_with_type_hinting() {
    let _closure = move || None::<String>;
}

#[test]
#[clone_on_capture]
fn method_expr() {
    let arr: Vec<String> = Vec::new();

    let _vec: Vec<()> = arr
        .into_iter()
        .map(|x| {
            let _a = move || x;
            let _b = x;
        })
        .collect();
}

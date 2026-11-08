//! Using `t!` before `init`. This binary never initializes.
use ezlz::t;

#[test]
#[should_panic(expected = "ezlz::init() must be called before using t!")]
fn t_panics_before_init() {
    let _ = t!("en", any.key);
}

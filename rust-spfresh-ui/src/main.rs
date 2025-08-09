use leptos::{mount_to_body, view};
use console_error_panic_hook::set_once;

mod app;

fn main() {
    set_once();
    mount_to_body(|| view! { <app::App/> });
}

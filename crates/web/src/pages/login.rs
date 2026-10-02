use leptos::prelude::*;

use crate::components::RedirectIfAuthenticated;

#[component]
pub fn LoginPage() -> impl IntoView {
    view! {
        <RedirectIfAuthenticated>
            <h1 class="text-2xl font-semibold">"登录"</h1>
        </RedirectIfAuthenticated>
    }
}

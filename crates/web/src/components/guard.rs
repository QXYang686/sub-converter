use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::state::{AuthStatus, AuthStore};

#[component]
pub fn RequireAuth(children: Children) -> impl IntoView {
    let store = AuthStore::expect();
    let navigate = use_navigate();
    Effect::new(move |_| {
        if matches!(store.status.get(), AuthStatus::Anonymous) {
            navigate("/login", Default::default());
        }
    });

    children()
}

#[component]
pub fn RedirectIfAuthenticated(children: Children) -> impl IntoView {
    let store = AuthStore::expect();
    let navigate = use_navigate();
    Effect::new(move |_| {
        if matches!(store.status.get(), AuthStatus::Authenticated(_)) {
            navigate("/", Default::default());
        }
    });

    children()
}

use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

use crate::state::{AuthStatus, AuthStore};

use super::spinner::Spinner;

#[component]
pub fn RequireAuth(children: ChildrenFn) -> impl IntoView {
    let store = AuthStore::expect();
    let navigate = use_navigate();
    Effect::new(move |_| {
        if matches!(store.status.get(), AuthStatus::Anonymous) {
            navigate("/login", Default::default());
        }
    });

    view! {
        {move || match store.status.get() {
            AuthStatus::Authenticated(_) => children(),
            _ => view! { <Spinner/> }.into_any(),
        }}
    }
}

#[component]
pub fn RedirectIfAuthenticated(children: ChildrenFn) -> impl IntoView {
    let store = AuthStore::expect();
    let navigate = use_navigate();
    Effect::new(move |_| {
        if matches!(store.status.get(), AuthStatus::Authenticated(_)) {
            navigate("/", Default::default());
        }
    });

    view! {
        {move || match store.status.get() {
            AuthStatus::Authenticated(_) => ().into_any(),
            _ => children(),
        }}
    }
}

use std::cell::RefCell;
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use leptos::prelude::*;
use leptos::task::spawn_local;
use send_wrapper::SendWrapper;

use contract::user::UserResponse;

use crate::api::{self, ApiError};

type RefreshFuture = Shared<LocalBoxFuture<'static, Result<(), ApiError>>>;

#[derive(Clone, PartialEq, Eq)]
pub enum AuthStatus {
    Loading,
    Authenticated(UserResponse),
    Anonymous,
}

#[derive(Clone)]
pub struct AuthStore {
    pub status: RwSignal<AuthStatus>,
    access_token: RwSignal<Option<String>>,
    refresh_in_flight: SendWrapper<Rc<RefCell<Option<RefreshFuture>>>>,
}

impl AuthStore {
    pub fn provide() -> Self {
        let store = Self {
            status: RwSignal::new(AuthStatus::Loading),
            access_token: RwSignal::new(None),
            refresh_in_flight: SendWrapper::new(Rc::new(RefCell::new(None))),
        };
        provide_context(store.clone());
        store
    }

    pub fn expect() -> Self {
        use_context::<Self>().expect("AuthStore was not provided")
    }

    pub fn access_token(&self) -> Option<String> {
        self.access_token.get_untracked()
    }

    pub fn bootstrap(&self) {
        let store = self.clone();
        spawn_local(async move {
            let _ = store.refresh().await;
        });
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<(), ApiError> {
        let auth = api::login(username, password).await?;
        self.set_session(auth.access_token, auth.user);
        Ok(())
    }

    pub async fn register(&self, username: &str, password: &str) -> Result<(), ApiError> {
        api::register(username, password).await?;
        self.login(username, password).await
    }

    pub async fn logout(&self) {
        let _ = api::logout().await;
        self.clear();
    }

    pub fn refresh(&self) -> RefreshFuture {
        if let Some(future) = self.refresh_in_flight.borrow().as_ref() {
            return future.clone();
        }

        let store = self.clone();
        let future = async move {
            match api::refresh().await {
                Ok(auth) => {
                    store.set_session(auth.access_token, auth.user);
                    Ok(())
                }
                Err(err) => {
                    store.clear();
                    Err(err)
                }
            }
        }
        .boxed_local()
        .shared();

        *self.refresh_in_flight.borrow_mut() = Some(future.clone());

        let slot = self.refresh_in_flight.clone();
        let wait = future.clone();
        spawn_local(async move {
            let _ = wait.await;
            *slot.borrow_mut() = None;
        });

        future
    }

    pub async fn current_user(&self) -> Result<UserResponse, ApiError> {
        let token = self.access_token().ok_or_else(ApiError::unauthenticated)?;
        match api::current_user(&token).await {
            Ok(user) => Ok(user),
            Err(err) if err.is_unauthorized() => {
                self.refresh().await?;
                let token = self.access_token().ok_or_else(ApiError::unauthenticated)?;
                api::current_user(&token).await
            }
            Err(err) => Err(err),
        }
    }

    fn set_session(&self, access_token: String, user: UserResponse) {
        self.access_token.set(Some(access_token));
        self.status.set(AuthStatus::Authenticated(user));
    }

    fn clear(&self) {
        self.access_token.set(None);
        self.status.set(AuthStatus::Anonymous);
    }
}

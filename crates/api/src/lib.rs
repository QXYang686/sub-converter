pub mod http;

#[cfg(target_arch = "wasm32")]
mod telemetry;

#[cfg(target_arch = "wasm32")]
mod worker_entry;

#[cfg(test)]
mod tests {
    #[test]
    fn contract_constraints_match_domain_rules() {
        assert_eq!(contract::USERNAME_MIN_LEN, user::USERNAME_MIN_LEN);
        assert_eq!(contract::USERNAME_MAX_LEN, user::USERNAME_MAX_LEN);
        assert_eq!(contract::PASSWORD_MIN_LEN, auth::domain::PASSWORD_MIN_LEN);
        assert_eq!(contract::PASSWORD_MAX_LEN, auth::domain::PASSWORD_MAX_LEN);
        assert_eq!(
            contract::SUBSCRIPTION_NAME_MIN_LEN,
            subscription::SUBSCRIPTION_NAME_MIN_LEN
        );
        assert_eq!(
            contract::SUBSCRIPTION_NAME_MAX_LEN,
            subscription::SUBSCRIPTION_NAME_MAX_LEN
        );
        assert_eq!(
            contract::SOURCE_URL_MAX_LEN,
            subscription::SOURCE_URL_MAX_LEN
        );
        assert_eq!(
            contract::PUBLICATION_MAX_SOURCES,
            subscription::PUBLICATION_MAX_SOURCES
        );
    }
}

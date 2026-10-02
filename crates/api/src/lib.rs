mod http;

#[cfg(target_arch = "wasm32")]
mod worker_entry;

#[cfg(test)]
mod tests {
    #[test]
    fn contract_constraints_match_domain_rules() {
        assert_eq!(contract::USERNAME_MIN_LEN, user::domain::USERNAME_MIN_LEN);
        assert_eq!(contract::USERNAME_MAX_LEN, user::domain::USERNAME_MAX_LEN);
        assert_eq!(contract::PASSWORD_MIN_LEN, user::domain::PASSWORD_MIN_LEN);
        assert_eq!(contract::PASSWORD_MAX_LEN, user::domain::PASSWORD_MAX_LEN);
    }
}

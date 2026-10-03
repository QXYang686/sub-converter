use auth::application::{
    LoginResult, PasskeyLoginOptions, PasskeyRegistrationOptions, PasskeyView, RefreshResult,
    UserView,
};
use contract::auth::AuthResponse;
use contract::passkey::{
    AuthenticatorSelection, CredentialDescriptor, PasskeyResponse, PubKeyCredParam,
    PublicKeyCredentialCreationOptions, PublicKeyCredentialRequestOptions, RelyingParty,
    UserEntity,
};
use contract::subscription::{
    ProtocolCountResponse, PublicationResponse, RuleProviderResponse, SourceResponse,
    SourceSnapshotResponse,
};
use contract::user::UserResponse;
use subscription::application::{
    PublicationView, RuleProviderView, SourceSnapshotView, SourceView,
};

const OPTION_TIMEOUT_MS: u32 = 300_000;

pub fn user_response(view: UserView) -> UserResponse {
    UserResponse {
        id: view.id,
        username: view.username,
    }
}

pub fn source_response(view: SourceView) -> SourceResponse {
    SourceResponse {
        id: view.id,
        name: view.name,
        url: view.url,
        enabled: view.enabled,
        created_at: view.created_at,
        updated_at: view.updated_at,
        snapshot: view.snapshot.map(snapshot_response),
    }
}

fn snapshot_response(view: SourceSnapshotView) -> SourceSnapshotResponse {
    SourceSnapshotResponse {
        fetched_at: view.fetched_at,
        proxy_count: view.proxy_count,
        group_count: view.group_count,
        rule_count: view.rule_count,
        protocol_counts: view
            .protocol_counts
            .into_iter()
            .map(|entry| ProtocolCountResponse {
                protocol: entry.protocol,
                count: entry.count,
            })
            .collect(),
        upload: view.upload,
        download: view.download,
        total: view.total,
        expire: view.expire,
        update_interval: view.update_interval,
        provider_name: view.provider_name,
        provider_url: view.provider_url,
        last_error: view.last_error,
    }
}

pub fn rule_provider_response(view: RuleProviderView) -> RuleProviderResponse {
    RuleProviderResponse {
        name: view.name,
        provider_type: view.provider_type,
        behavior: view.behavior,
        url: view.url,
        interval: view.interval,
        rule_count: view.rule_count,
        fetched_at: view.fetched_at,
        has_snapshot: view.has_snapshot,
        last_error: view.last_error,
    }
}

pub fn publication_response(view: PublicationView) -> PublicationResponse {
    PublicationResponse {
        id: view.id,
        name: view.name,
        secret: view.secret,
        enabled: view.enabled,
        expires_at: view.expires_at,
        source_ids: view.source_ids,
        created_at: view.created_at,
        updated_at: view.updated_at,
    }
}

pub fn auth_response(result: LoginResult) -> AuthResponse {
    AuthResponse {
        access_token: result.access_token,
        access_token_expires_at: result.access_token_expires_at,
        user: user_response(result.user),
    }
}

pub fn refresh_auth_response(result: RefreshResult) -> AuthResponse {
    AuthResponse {
        access_token: result.access_token,
        access_token_expires_at: result.access_token_expires_at,
        user: user_response(result.user),
    }
}

pub fn creation_options(options: PasskeyRegistrationOptions) -> PublicKeyCredentialCreationOptions {
    PublicKeyCredentialCreationOptions {
        challenge: options.challenge,
        rp: RelyingParty {
            id: options.rp_id,
            name: options.rp_name,
        },
        user: UserEntity {
            id: options.user_handle,
            name: options.username.clone(),
            display_name: options.username,
        },
        pub_key_cred_params: vec![
            PubKeyCredParam {
                kind: "public-key".to_string(),
                alg: -7,
            },
            PubKeyCredParam {
                kind: "public-key".to_string(),
                alg: -8,
            },
        ],
        timeout: OPTION_TIMEOUT_MS,
        attestation: "none".to_string(),
        authenticator_selection: AuthenticatorSelection {
            resident_key: "preferred".to_string(),
            user_verification: "preferred".to_string(),
        },
        exclude_credentials: options
            .exclude_credential_ids
            .into_iter()
            .map(|id| CredentialDescriptor {
                kind: "public-key".to_string(),
                id,
                transports: None,
            })
            .collect(),
    }
}

pub fn request_options(options: PasskeyLoginOptions) -> PublicKeyCredentialRequestOptions {
    PublicKeyCredentialRequestOptions {
        challenge: options.challenge,
        rp_id: options.rp_id,
        timeout: OPTION_TIMEOUT_MS,
        user_verification: if options.user_verification {
            "required".to_string()
        } else {
            "preferred".to_string()
        },
        allow_credentials: options
            .allow_credential_ids
            .into_iter()
            .map(|id| CredentialDescriptor {
                kind: "public-key".to_string(),
                id,
                transports: None,
            })
            .collect(),
    }
}

pub fn passkey_response(view: PasskeyView) -> PasskeyResponse {
    PasskeyResponse {
        id: view.id,
        label: view.label,
        transports: view.transports,
        created_at: view.created_at,
        last_used_at: view.last_used_at,
    }
}

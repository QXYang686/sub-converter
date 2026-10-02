use std::sync::Arc;

use user::UserId;

use crate::domain::PublicationRepository;

use super::dto::PublicationView;
use super::error::AppError;

pub struct ListPublicationsHandler {
    publications: Arc<dyn PublicationRepository>,
}

impl ListPublicationsHandler {
    pub fn new(publications: Arc<dyn PublicationRepository>) -> Self {
        Self { publications }
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle(&self, user_id: &UserId) -> Result<Vec<PublicationView>, AppError> {
        let publications = self.publications.list_by_user(user_id).await?;
        Ok(publications.iter().map(PublicationView::from).collect())
    }
}

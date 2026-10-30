//! [`Member`]: a forge identity and its role.

use facet::Facet;

use crate::entity::Entity;

#[derive(Debug, Clone, Facet)]
pub struct Member {
    pub id: String,
    pub signing_key: String,
    pub role: String,
}

#[derive(Debug, Facet)]
pub struct StoredMember {
    signing_key: String,
    role: String,
}

impl Entity for Member {
    const KIND: &'static str = "member";
    type Stored = StoredMember;

    fn id(&self) -> &str {
        &self.id
    }

    fn to_stored(&self) -> StoredMember {
        StoredMember {
            signing_key: self.signing_key.clone(),
            role: self.role.clone(),
        }
    }

    fn from_stored(id: String, stored: StoredMember) -> Self {
        Self {
            id,
            signing_key: stored.signing_key,
            role: stored.role,
        }
    }
}

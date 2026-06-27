use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct CounterpartyDto {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::counterparty::Model> for CounterpartyDto {
    fn from(c: entity::counterparty::Model) -> Self {
        CounterpartyDto {
            id: c.id,
            kind: c.kind,
            name: c.name,
            contact_name: c.contact_name,
            email: c.email,
            phone: c.phone,
            website: c.website,
            address: c.address,
            notes: c.notes,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct NoteDto {
    pub id: Uuid,
    pub counterparty_id: Uuid,
    pub author_user_id: Option<Uuid>,
    pub body: String,
    pub created_at: String,
}

impl From<entity::counterparty_note::Model> for NoteDto {
    fn from(n: entity::counterparty_note::Model) -> Self {
        NoteDto {
            id: n.id,
            counterparty_id: n.counterparty_id,
            author_user_id: n.author_user_id,
            body: n.body,
            created_at: n.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CounterpartyDetailDto {
    #[serde(flatten)]
    pub entity: CounterpartyDto,
    /// Timestamped note history (the inline `notes` field is a short summary).
    pub notes_log: Vec<NoteDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateCounterpartyReq {
    pub kind: String,
    pub name: String,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateCounterpartyReq {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub contact_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddNoteReq {
    pub body: String,
}

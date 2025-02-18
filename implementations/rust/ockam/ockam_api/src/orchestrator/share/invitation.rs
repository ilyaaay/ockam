use crate::address::extract_address_value;
use crate::cli_state::{EnrollmentTicket, ExportedEnrollmentTicket};
use crate::date::is_expired;
use crate::error::ApiError;
use crate::orchestrator::email_address::EmailAddress;
use crate::output::Output;
use minicbor::{CborLen, Decode, Encode};
use ockam::identity::Identifier;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};

#[derive(Clone, Debug, Eq, PartialEq, Decode, Encode, CborLen, Deserialize, Serialize)]
#[cbor(index_only)]
#[rustfmt::skip]
pub enum RoleInShare {
    #[n(0)] Admin,
    #[n(1)] Guest,
    #[n(2)] Service,
}

impl Display for RoleInShare {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::Guest => write!(f, "guest"),
            Self::Service => write!(f, "service_user"),
        }
    }
}

impl FromStr for RoleInShare {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "guest" => Ok(Self::Guest),
            "service_user" => Ok(Self::Service),
            other => Err(format!("unknown role: {other}")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Decode, Encode, CborLen, Deserialize, Serialize)]
#[cbor(index_only)]
#[rustfmt::skip]
pub enum ShareScope {
    #[n(0)] Project,
    #[n(1)] Service,
    #[n(2)] Space,
}

impl Display for ShareScope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            ShareScope::Project => write!(f, "project"),
            ShareScope::Service => write!(f, "service"),
            ShareScope::Space => write!(f, "space"),
        }
    }
}

impl FromStr for ShareScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "project" => Ok(Self::Project),
            "service" => Ok(Self::Service),
            "space" => Ok(Self::Space),
            other => Err(format!("unknown scope: {other}")),
        }
    }
}

#[derive(Clone, Debug, Encode, Decode, CborLen, Deserialize, Serialize)]
#[cbor(map)]
#[rustfmt::skip]
pub struct InvitationWithAccess {
    #[n(1)] pub invitation: ReceivedInvitation,
    #[n(2)] pub service_access_details: Option<ServiceAccessDetails>,
}

impl PartialEq for InvitationWithAccess {
    fn eq(&self, other: &Self) -> bool {
        self.invitation.id == other.invitation.id
    }
}

impl Eq for InvitationWithAccess {}

#[derive(Clone, Debug, Encode, Decode, CborLen, Deserialize, Serialize, PartialEq)]
#[cbor(map)]
#[rustfmt::skip]
pub struct ReceivedInvitation {
    #[n(1)] pub id: String,
    #[n(2)] pub expires_at: String,
    #[n(3)] pub grant_role: RoleInShare,
    #[n(4)] pub owner_email: EmailAddress,
    #[n(5)] pub scope: ShareScope,
    #[n(6)] pub target_id: String,
    #[n(7)] pub ignored: bool,
}

impl ReceivedInvitation {
    pub fn is_expired(&self) -> ockam_core::Result<bool> {
        is_expired(&self.expires_at)
    }
}

impl Output for ReceivedInvitation {
    fn item(&self) -> crate::Result<String> {
        Ok(format!(
            "{}\n  scope: {} target_id: {} (expires {})",
            self.id, self.scope, self.target_id, self.expires_at
        ))
    }
}

#[derive(Clone, Debug, Encode, Decode, CborLen, Deserialize, Serialize, PartialEq)]
#[cbor(map)]
#[rustfmt::skip]
pub struct SentInvitation {
    #[n(1)] pub id: String,
    #[n(2)] pub expires_at: String,
    #[n(3)] pub grant_role: RoleInShare,
    #[n(4)] pub owner_id: usize,
    #[n(5)] pub recipient_email: EmailAddress,
    #[n(6)] pub remaining_uses: usize,
    #[n(7)] pub scope: ShareScope,
    #[n(8)] pub target_id: String,
    #[n(9)] pub recipient_id: usize,
    #[n(10)] pub access_details: Option<ServiceAccessDetails>,
}

impl SentInvitation {
    pub fn is_expired(&self) -> ockam_core::Result<bool> {
        is_expired(&self.expires_at)
    }
}

impl Output for SentInvitation {
    fn item(&self) -> crate::Result<String> {
        Ok(format!(
            "{}\n  scope: {} target_id: {} (expires {}) for: {:?}",
            self.id, self.scope, self.target_id, self.expires_at, self.recipient_email,
        ))
    }
}

#[derive(Clone, Debug, Encode, Decode, CborLen, Deserialize, Serialize, PartialEq)]
#[cbor(map)]
#[rustfmt::skip]
pub struct ServiceAccessDetails {
    #[n(1)] pub project_identity: Identifier,
    #[n(2)] pub project_route: String,
    #[n(3)] pub project_authority_identity: Identifier,
    #[n(4)] pub project_authority_route: String,
    #[n(5)] pub shared_node_identity: Identifier,
    #[n(6)] pub shared_node_route: String,
    #[n(7)] pub enrollment_ticket: String, // encoded as with CLI output/input
}

impl ServiceAccessDetails {
    pub async fn enrollment_ticket(&self) -> ockam_core::Result<EnrollmentTicket> {
        Ok(ExportedEnrollmentTicket::from_str(&self.enrollment_ticket)
            .map_err(|e| ApiError::core(e.to_string()))?
            .import()
            .await?)
    }

    pub fn service_name(&self) -> Result<String, ApiError> {
        extract_address_value(&self.shared_node_route)
    }
}

#[cfg(test)]
mod test {
    use crate::date::is_expired;
    use time::format_description::well_known::Iso8601;
    use time::OffsetDateTime;

    #[test]
    fn test_is_expired() {
        let now = OffsetDateTime::now_utc();

        let non_expired = now + time::Duration::days(1);
        let non_expired_str = non_expired.format(&Iso8601::DEFAULT).unwrap();
        assert!(!is_expired(&non_expired_str).unwrap());

        let expired = now - time::Duration::days(1);
        let expired_str = expired.format(&Iso8601::DEFAULT).unwrap();
        assert!(is_expired(&expired_str).unwrap());

        // The following test cases are just to validate that it can parse the string correctly
        assert!(is_expired("2020-09-12T15:07:14.00").unwrap());
        assert!(is_expired("2020-09-12T15:07:14.00Z").unwrap());
    }
}

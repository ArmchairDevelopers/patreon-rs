use crate::{ApiError, PatreonError, PatreonResult};
use chrono::{DateTime, Utc};
use serde_derive::Deserialize;
use serde_derive::Serialize;
use std::sync::Arc;
use url::Url;

static BASE_URI: &str = "https://www.patreon.com";

#[derive(Debug, Default)]
pub struct PatreonApi {
    pub access_token: String,
    pub agent: Arc<reqwest::Client>,
}

impl PatreonApi {
    pub async fn current_user(&self) -> PatreonResult<User> {
        let mut url = Url::parse(BASE_URI).unwrap();
        url.set_path("/api/oauth2/api/current_user");
        let request = self.agent.get(url);
        self.call_data(request).await
    }

    pub async fn identity(&self) -> PatreonResult<User> {
        self.call_data(self.identity_request(None)).await
    }

    pub async fn identity_and_member(&self) -> PatreonResult<(User, Vec<Member>)> {
        self.call_data_and_include(self.identity_request(IdentityIncldue::Memberships))
            .await
    }

    fn identity_request(
        &self,
        include: impl Into<Option<IdentityIncldue>>,
    ) -> reqwest::RequestBuilder {
        let mut url = Url::parse(BASE_URI).unwrap();
        url.set_path("api/oauth2/v2/identity");
        url.query_pairs_mut().append_pair(
            "fields[user]",
            "first_name,last_name,full_name,vanity,email,about,image_url,thumb_url,created,url",
        );
        let include = include.into();
        if let Some(include) = include {
            url.query_pairs_mut()
                .append_pair("include", include.as_str());
            match include {
                IdentityIncldue::Memberships => {
                    url.query_pairs_mut().append_pair(
                        "fields[member]",
                        "campaign_lifetime_support_cents,currently_entitled_amount_cents,email,full_name,is_follower,last_charge_date,last_charge_status,lifetime_support_cents,next_charge_date,note,patron_status,pledge_cadence,pledge_relationship_start,will_pay_amount_cents",
                    );
                }
                IdentityIncldue::Campaign => {
                    url.query_pairs_mut()
                        .append_pair("fields[campaign]", "created_at,creation_name,discord_server_id,google_analytics_id,has_rss,has_sent_rss_notify,image_small_url,image_url,is_charged_immediately,is_monthly,is_nsfw,main_video_embed,main_video_url,one_liner,patron_count,pay_per_name,pledge_url,published_at,rss_artwork_url,rss_feed_title,show_earnings,summary,thanks_embed,thanks_msg,thanks_video_url,url,vanity");
                }
            }
        }
        self.agent.get(url)
    }

    async fn api_call(&self, request: reqwest::RequestBuilder) -> PatreonResult<String> {
        let request = request
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("User-Agent", "Patreon-rust")
            .build()?;
        tracing::debug!("REQUEST : {} : {}", request.method(), request.url());
        let response = self.agent.execute(request).await?;
        let status = response.status();
        let text = response.text().await?;
        tracing::debug!("RESPONSE : {status} : {text}");
        if status.is_success() {
            Ok(text)
        } else {
            Err(PatreonError::PatreonApi(
                status,
                serde_json::from_str::<ApiErrorResponse>(text.as_str())?.errors,
            ))
        }
    }

    async fn call_data<T: for<'de> serde::Deserialize<'de>>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> PatreonResult<T> {
        let json = self.api_call(request).await?;
        Ok(serde_json::from_str::<DocResponse<T>>(json.as_str())?.data)
    }

    async fn call_data_and_include<
        D: for<'de> serde::Deserialize<'de>,
        I: for<'de> serde::Deserialize<'de>,
    >(
        &self,
        request: reqwest::RequestBuilder,
    ) -> PatreonResult<(D, Vec<I>)> {
        let json = self.api_call(request).await?;
        let response = serde_json::from_str::<DocResponseInclude<D, I>>(json.as_str())?;
        Ok((response.data, response.include))
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DocResponse<D> {
    data: D,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DocResponseInclude<D, I> {
    data: D,
    include: Vec<I>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiDocument<A> {
    #[serde(rename = "type")]
    pub document_type: String,
    pub id: String,
    pub attributes: A,
}

pub type User = ApiDocument<UserAttributes>;
pub type Member = ApiDocument<MemberAttributes>;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserAttributes {
    pub first_name: String,
    pub last_name: String,
    pub full_name: String,
    pub vanity: Option<String>,
    pub email: String,
    pub about: Option<String>,
    pub facebook_id: Option<String>,
    pub image_url: String,
    pub thumb_url: String,
    pub youtube: Option<String>,
    pub twitter: Option<String>,
    pub facebook: Option<String>,
    pub created: DateTime<Utc>,
    pub url: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberAttributes {
    pub campaign_lifetime_support_cents: i64,
    pub currently_entitled_amount_cents: i64,
    pub email: Option<String>,
    pub full_name: String,
    pub is_follower: bool,
    pub last_charge_date: DateTime<Utc>,
    pub last_charge_status: Option<LastChrgeStatus>,
    pub lifetime_support_cents: i64,
    pub next_charge_date: DateTime<Utc>,
    pub note: String,
    pub patron_status: Option<PatronStatus>,
    pub pledge_cadence: i64,
    pub pledge_relationship_start: DateTime<Utc>,
    pub will_pay_amount_cents: i64,
}

#[derive(Serialize, Deserialize)]
struct ApiErrorResponse {
    pub errors: Vec<ApiError>,
}

macro_rules! enum_str {
    ($name:ident { $($variant:ident($str:expr), )* }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant,)*
        }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $( $name::$variant => $str, )*
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $( $name::$variant => write!(f,"{}",$str), )*
                }
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: ::serde::Serializer,
            {
                // 将枚举序列化为字符串。
                serializer.serialize_str(match *self {
                    $( $name::$variant => $str, )*
                })
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: ::serde::Deserializer<'de>,
            {
                struct Visitor;

                impl<'de> ::serde::de::Visitor<'de> for Visitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                        write!(formatter, "a string for {}", stringify!($name))
                    }

                    fn visit_str<E>(self, value: &str) -> Result<$name, E>
                        where E: ::serde::de::Error,
                    {
                        match value {
                            $( $str => Ok($name::$variant), )*
                            _ => Err(E::invalid_value(::serde::de::Unexpected::Other(
                                &format!("unknown {} variant: {}", stringify!($name), value)
                            ), &self)),
                        }
                    }
                }

                // 从字符串反序列化枚举。
                deserializer.deserialize_str(Visitor)
            }
        }
    };
    ($name:ident { $($variant:ident,)* } ) => {
        enum_str!(
            $name {
             $($variant(stringify!($variant)),)*
            }
        );
    };
}

enum_str!(IdentityIncldue {
    Memberships("memberships"),
    Campaign("campaign"),
});

enum_str!(LastChrgeStatus {
    Paid,
    Declined,
    Deleted,
    Pending,
    Refunded,
    Fraud,
    Other,
});

enum_str!(PatronStatus {
    ActivePatron("active_patron"),
    DeclinedPatron("declined_patron"),
    FormerPatron("former_patron"),
});
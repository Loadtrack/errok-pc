use crate::core::{format_date, parse_available, Catalog, College, Day, OpecDaysResponse};
use chrono::NaiveDate;
use reqwest::{header, RequestBuilder, Response, StatusCode};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;
use thiserror::Error;
use url::Url;

const BASE_URL: &str = "https://errok.ru";

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Сервер не ответил вовремя")]
    Timeout,
    #[error("Нет соединения с сервером: {0}")]
    Network(String),
    #[error("Сервер вернул ошибку {0}")]
    Http(u16),
    #[error("Слишком много запросов, повтор через {0} с")]
    RateLimited(u64),
    #[error("Не удалось разобрать ответ сервера: {0}")]
    Parse(String),
}

impl ApiError {
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout)
    }

    pub fn retry_after(&self) -> Option<u64> {
        match self {
            Self::RateLimited(seconds) => Some(*seconds),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Fetch<T> {
    NotModified,
    Fresh { data: T, etag: Option<String> },
}

#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
}

impl ApiClient {
    pub fn new() -> Result<Self, ApiError> {
        let client = reqwest::Client::builder()
            .user_agent(format!("errok-desktop/{} (Windows)", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .https_only(true)
            .build()
            .map_err(|error| ApiError::Network(error.to_string()))?;
        Ok(Self { client })
    }

    async fn execute(&self, request: RequestBuilder) -> Result<Response, ApiError> {
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                ApiError::Timeout
            } else {
                ApiError::Network(error.to_string())
            }
        })?;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            let retry = response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(ApiError::RateLimited(retry));
        }
        if response.status().is_server_error() || response.status().is_client_error() {
            return Err(ApiError::Http(response.status().as_u16()));
        }
        Ok(response)
    }

    fn conditional(request: RequestBuilder, etag: Option<&str>) -> RequestBuilder {
        match etag {
            Some(etag) => request.header(header::IF_NONE_MATCH, etag),
            None => request,
        }
    }

    fn etag(response: &Response) -> Option<String> {
        response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }

    pub async fn available(
        &self,
        college: College,
        etag: Option<&str>,
    ) -> Result<Fetch<BTreeMap<String, String>>, ApiError> {
        let path = match college {
            College::Opec => "/available.json",
            College::Omaep => "/d/omaep/daily/available.json",
        };
        let request = Self::conditional(self.client.get(format!("{BASE_URL}{path}")), etag);
        let response = self.execute(request).await?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(Fetch::NotModified);
        }
        let etag = Self::etag(&response);
        let text = response.text().await.map_err(|error| ApiError::Network(error.to_string()))?;
        let data = parse_available(&text).map_err(|error| ApiError::Parse(error.to_string()))?;
        Ok(Fetch::Fresh { data, etag })
    }

    pub async fn catalog(&self, college: College) -> Result<Catalog, ApiError> {
        match college {
            College::Opec => {
                let response = self
                    .execute(self.client.get(format!("{BASE_URL}/api/fav_days.php?catalog=1")))
                    .await?;
                let catalog: OpecCatalog = response.json().await.map_err(|error| ApiError::Parse(error.to_string()))?;
                if !catalog.ok {
                    return Err(ApiError::Parse("Справочник вернул ok: false".into()));
                }
                Ok(Catalog { groups: catalog.groups, teachers: catalog.teachers })
            }
            College::Omaep => {
                let response = self
                    .execute(self.client.get(format!("{BASE_URL}/d/omaep/weeks/available.json")))
                    .await?;
                let groups: BTreeMap<String, OmaepCatalogGroup> = response
                    .json()
                    .await
                    .map_err(|error| ApiError::Parse(error.to_string()))?;
                Ok(Catalog {
                    groups: groups.into_values().map(|group| group.name).collect(),
                    teachers: vec![],
                })
            }
        }
    }

    pub async fn opec_days(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        groups: &[String],
        teachers: &[String],
    ) -> Result<OpecDaysResponse, ApiError> {
        if (to - from).num_days() > 15 || to < from {
            return Err(ApiError::Parse("Диапазон дат больше 16 дней".into()));
        }
        let mut url = Url::parse(&format!("{BASE_URL}/api/fav_days.php"))
            .map_err(|error| ApiError::Parse(error.to_string()))?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("from", &format_date(from));
            query.append_pair("to", &format_date(to));
            for group in groups.iter().take(8) {
                query.append_pair("g[]", group);
            }
            for teacher in teachers.iter().take(8) {
                query.append_pair("t[]", teacher);
            }
        }
        let response = self.execute(self.client.get(url)).await?;
        let result: OpecDaysResponse = response.json().await.map_err(|error| ApiError::Parse(error.to_string()))?;
        if !result.ok {
            return Err(ApiError::Parse("Сервер не вернул расписание".into()));
        }
        Ok(result)
    }

    pub async fn omaep_day(&self, date: NaiveDate, etag: Option<&str>) -> Result<Fetch<Day>, ApiError> {
        let path = format!("/d/omaep/daily/schedule_{}.json", format_date(date));
        let request = Self::conditional(self.client.get(format!("{BASE_URL}{path}")), etag);
        let response = self.execute(request).await?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(Fetch::NotModified);
        }
        let etag = Self::etag(&response);
        let day = response.json().await.map_err(|error| ApiError::Parse(error.to_string()))?;
        Ok(Fetch::Fresh { data: day, etag })
    }
}

#[derive(Deserialize)]
struct OpecCatalog {
    ok: bool,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    teachers: Vec<String>,
}

#[derive(Deserialize)]
struct OmaepCatalogGroup {
    name: String,
}

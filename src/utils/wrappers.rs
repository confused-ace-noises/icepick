use std::{ops::{Deref, DerefMut}, path::Display, str::FromStr};
use chrono::{DateTime as ChronoDatetime, Local};
use tokio_rusqlite::ToSql;
use url::Url;
use anyhow::Result;

#[derive(Clone, Debug)]
pub struct W<T>(T);

impl<T> Deref for W<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for W<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub type DateTime = W<ChronoDatetime<Local>>;

impl DateTime {
    #[must_use]
    pub fn now() -> Self {
        let now = Local::now();
        W(now)
    }
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted = self.format("%Y-%m-%d %H:%M:%S").to_string();

        f.write_str(formatted.as_str())
    }
}

pub trait ToUrlStr {
    fn to_url(&self) -> Result<Url>;
}

impl ToUrlStr for str {
    fn to_url(&self) -> Result<Url> {
        let tmp = Url::from_str(self)?;
        Ok(tmp)
    }
}

pub trait ToStorable {
    type Output: ToSql;
    fn to_storable(self) -> Self::Output;
}

impl ToStorable for ChronoDatetime<Local> {
    type Output = String;
    fn to_storable(self) -> Self::Output {
        self.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}